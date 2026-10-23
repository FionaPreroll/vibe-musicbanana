//! What guards the cover proxy (src/covers.rs) against a misbehaving Cover Art
//! Archive or MusicBrainz: redirects, oversized answers, a full search queue
//! and the size limit of the cache. Against stand-ins on 127.0.0.1; another
//! host is reached as `localhost`, which the stand-in tells apart.

use std::{
    io::Write as _,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, SystemTime},
};

use axum::{
    Router,
    body::{Body, Bytes},
    extract::{Path as UrlPath, State},
    http::{HeaderMap, header},
    response::{IntoResponse, Redirect, Response},
    routing::get,
};
use futures_util::stream;
use musicbanana::covers::{Covers, Lookup, MAX_BYTES, MAX_SEARCH_BYTES, Settings};
use reqwest::Url;
use tokio::net::TcpListener;
use uuid::Uuid;

/// A JPEG as far as the server looks.
const JPEG: &[u8] = &[0xff, 0xd8, 0xff, 0xe0, 0, 16, b'J', b'F', b'I', b'F', 0];

/// How the stand-in archive answers.
#[derive(Clone, Copy, Default, PartialEq)]
enum Mode {
    /// Off to /download on the same host, as the real archive does to archive.org.
    #[default]
    Normal,
    /// Off to another host (`localhost`), with a secret in the query.
    Elsewhere,
    /// Off to itself, again and again.
    Loop,
    /// A cover larger than allowed, sent in pieces without a length.
    HugeChunked,
    /// A cover larger than allowed, with its length.
    HugeSized,
    /// A cover exactly as large as allowed, in pieces.
    LargestChunked,
}

#[derive(Default)]
struct Upstream {
    mode: Mutex<Mode>,
    port: Mutex<u16>,
    /// Requests to the archive.
    asked: AtomicUsize,
    /// Requests that reached the other host.
    elsewhere: AtomicUsize,
    searches: AtomicUsize,
    /// How MusicBrainz answers: "huge", "redirect", or found nothing.
    search_mode: Mutex<&'static str>,
}

fn host_of(headers: &HeaderMap) -> String {
    headers[header::HOST].to_str().unwrap().to_owned()
}

fn chunked(total: usize) -> Body {
    let mut first = JPEG.to_vec();
    first.resize(64 * 1024, b'x');
    let rest = total - first.len();
    let chunks = std::iter::once(first)
        .chain((0..rest / (64 * 1024)).map(|_| vec![b'x'; 64 * 1024]))
        .chain(std::iter::once(vec![b'x'; rest % (64 * 1024)]))
        .map(|chunk| Ok::<_, std::io::Error>(Bytes::from(chunk)));
    Body::from_stream(stream::iter(chunks))
}

async fn archive(
    State(up): State<Arc<Upstream>>,
    headers: HeaderMap,
    UrlPath((mbid, file)): UrlPath<(String, String)>,
) -> Response {
    if host_of(&headers).starts_with("localhost") {
        up.elsewhere.fetch_add(1, Ordering::SeqCst);
        return ([(header::CONTENT_TYPE, "image/jpeg")], JPEG).into_response();
    }
    up.asked.fetch_add(1, Ordering::SeqCst);
    let port = *up.port.lock().unwrap();
    match *up.mode.lock().unwrap() {
        Mode::Normal => Redirect::temporary(&format!("/download/{mbid}/{file}")).into_response(),
        Mode::Elsewhere => Redirect::temporary(&format!(
            "http://localhost:{port}/release-group/{mbid}/{file}?token=s3cret"
        ))
        .into_response(),
        Mode::Loop => Redirect::temporary(&format!("/release-group/{mbid}/{file}")).into_response(),
        Mode::HugeChunked => chunked(MAX_BYTES + 1).into_response(),
        Mode::LargestChunked => chunked(MAX_BYTES).into_response(),
        Mode::HugeSized => {
            let mut body = JPEG.to_vec();
            body.resize(MAX_BYTES + 1, b'x');
            body.into_response()
        }
    }
}

async fn download(UrlPath((mbid, _)): UrlPath<(String, String)>) -> Response {
    let mut bytes = JPEG.to_vec();
    bytes.extend_from_slice(mbid.as_bytes());
    ([(header::CONTENT_TYPE, "image/jpeg")], bytes).into_response()
}

async fn search(State(up): State<Arc<Upstream>>) -> Response {
    up.searches.fetch_add(1, Ordering::SeqCst);
    let port = *up.port.lock().unwrap();
    match *up.search_mode.lock().unwrap() {
        "huge" => {
            let mut body = br#"{"release-groups": [], "padding": ""#.to_vec();
            body.resize(MAX_SEARCH_BYTES + 10, b'x');
            body.extend_from_slice(br#""}"#);
            body.into_response()
        }
        "redirect" => Redirect::temporary(&format!("http://localhost:{port}/release-group/x/y"))
            .into_response(),
        _ => r#"{"release-groups": []}"#.into_response(),
    }
}

async fn upstream() -> (Arc<Upstream>, Url) {
    let up = Arc::new(Upstream::default());
    let app = Router::new()
        .route("/release-group/{mbid}/{file}", get(archive))
        .route("/download/{mbid}/{file}", get(download))
        .route("/ws/2/release-group", get(search))
        .with_state(up.clone());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    *up.port.lock().unwrap() = port;
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (
        up,
        Url::parse(&format!("http://127.0.0.1:{port}/")).unwrap(),
    )
}

fn cache_dir() -> PathBuf {
    std::env::temp_dir().join(format!("musicbanana-hardening-{}", Uuid::new_v4()))
}

fn settings(url: &Url, dir: &Path) -> Settings {
    Settings {
        enabled: true,
        search_interval: Duration::ZERO,
        ..Settings::off(dir).with_upstream(url)
    }
}

fn mbid(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

/// Log lines written while the guard lives, on this thread.
#[derive(Clone, Default)]
struct Log(Arc<Mutex<Vec<u8>>>);

impl std::io::Write for Log {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().write(buf)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Log {
    fn capture(&self) -> tracing::subscriber::DefaultGuard {
        let log = self.clone();
        let subscriber = tracing_subscriber::fmt()
            .with_writer(move || log.clone())
            .with_ansi(false)
            .with_max_level(tracing::Level::DEBUG)
            .finish();
        tracing::subscriber::set_default(subscriber)
    }

    fn text(&self) -> String {
        String::from_utf8(self.0.lock().unwrap().clone()).unwrap()
    }
}

#[tokio::test(flavor = "current_thread")]
async fn redirects_elsewhere_are_refused_logged_and_remembered() {
    let (up, url) = upstream().await;
    *up.mode.lock().unwrap() = Mode::Elsewhere;
    let dir = cache_dir();
    let covers = Covers::new(settings(&url, &dir));
    let log = Log::default();
    let _guard = log.capture();

    assert!(covers.front(mbid(1), 250).await.is_none());
    assert_eq!(
        up.elsewhere.load(Ordering::SeqCst),
        0,
        "followed the redirect"
    );
    let text = log.text();
    let line = text
        .lines()
        .find(|l| l.contains("refused redirect"))
        .unwrap_or_else(|| panic!("nothing logged: {text}"));
    let port = *up.port.lock().unwrap();
    assert!(line.contains("WARN"), "{line}");
    assert!(
        line.contains(&format!(
            "to http://localhost:{port}/release-group/{}/front-250",
            mbid(1)
        )),
        "{line}"
    );
    assert!(
        line.contains(&format!("from http://127.0.0.1:{port}/")),
        "{line}"
    );
    assert!(
        !text.contains("s3cret") && !text.contains("token"),
        "{text}"
    );

    // Taken as no cover: not asked again for a while.
    assert!(dir.join(format!("caa/{}-250.missing", mbid(1))).exists());
    assert!(covers.front(mbid(1), 250).await.is_none());
    assert_eq!(up.asked.load(Ordering::SeqCst), 1);
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test(flavor = "current_thread")]
async fn redirect_loops_end() {
    let (up, url) = upstream().await;
    *up.mode.lock().unwrap() = Mode::Loop;
    let dir = cache_dir();
    let covers = Covers::new(settings(&url, &dir));
    let log = Log::default();
    let _guard = log.capture();

    assert!(covers.front(mbid(2), 250).await.is_none());
    // The first request and 5 redirects followed, the 6th refused.
    assert_eq!(up.asked.load(Ordering::SeqCst), 6);
    assert!(log.text().contains("too many redirects"), "{}", log.text());
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn the_normal_way_to_archive_org_still_works() {
    let (_, url) = upstream().await;
    let dir = cache_dir();
    let covers = Covers::new(settings(&url, &dir));
    let image = covers.front(mbid(3), 500).await.expect("a cover");
    assert!(image.bytes.starts_with(JPEG));
    assert_eq!(image.content_type, "image/jpeg");
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn oversized_covers_are_dropped_with_or_without_a_length() {
    let (up, url) = upstream().await;
    let dir = cache_dir();
    let covers = Covers::new(settings(&url, &dir));
    for (n, mode) in [(4, Mode::HugeChunked), (5, Mode::HugeSized)] {
        *up.mode.lock().unwrap() = mode;
        assert!(covers.front(mbid(n), 250).await.is_none());
        // A failure, not "no cover": nothing kept, asked again next time.
        assert!(!dir.join(format!("caa/{}-250", mbid(n))).exists());
        assert!(!dir.join(format!("caa/{}-250.missing", mbid(n))).exists());
    }
    let asked = up.asked.load(Ordering::SeqCst);
    assert!(covers.front(mbid(4), 250).await.is_none());
    assert_eq!(up.asked.load(Ordering::SeqCst), asked + 1);

    // Exactly the limit is still fine.
    *up.mode.lock().unwrap() = Mode::LargestChunked;
    let image = covers.front(mbid(6), 250).await.expect("the largest cover");
    assert_eq!(image.bytes.len(), MAX_BYTES);
    let _ = std::fs::remove_dir_all(dir);
}

/// Waits for the background searches; how many are left after two seconds.
async fn search_until_done(covers: &Covers) -> usize {
    for _ in 0..200 {
        if covers.searches_pending() == 0 {
            return 0;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    covers.searches_pending()
}

#[tokio::test]
async fn musicbrainz_is_held_to_size_and_never_followed() {
    let (up, url) = upstream().await;
    let dir = cache_dir();
    let covers = Covers::new(Settings {
        lookup: true,
        ..settings(&url, &dir)
    });
    for (artist, mode) in [("Huge", "huge"), ("Moved", "redirect")] {
        *up.search_mode.lock().unwrap() = mode;
        assert!(matches!(
            covers.looked_up(artist, "Album").await,
            Lookup::Queued
        ));
        assert_eq!(search_until_done(&covers).await, 0);
        // Failed searches are not kept: asked again on the next view.
        assert!(matches!(
            covers.looked_up(artist, "Album").await,
            Lookup::Queued
        ));
        search_until_done(&covers).await;
    }
    assert_eq!(up.elsewhere.load(Ordering::SeqCst), 0);
    assert_eq!(up.searches.load(Ordering::SeqCst), 4);
    assert!(!dir.join("musicbrainz").exists());

    // A normal answer is kept.
    *up.search_mode.lock().unwrap() = "";
    covers.looked_up("Kept", "Album").await;
    search_until_done(&covers).await;
    assert!(matches!(
        covers.looked_up("Kept", "Album").await,
        Lookup::Done(None)
    ));
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test(flavor = "current_thread")]
async fn the_search_queue_has_a_limit() {
    let (up, url) = upstream().await;
    let dir = cache_dir();
    let covers = Covers::new(Settings {
        lookup: true,
        search_queue: 2,
        // The first search runs at once, the next only after an hour.
        search_interval: Duration::from_secs(3600),
        ..settings(&url, &dir)
    });
    for n in 0..50 {
        covers.looked_up(&format!("Artist {n}"), "Album").await;
    }
    // At most the one being searched and the two waiting.
    let pending = covers.searches_pending();
    assert!((1..=3).contains(&pending), "{pending} pending");
    // Those turned away come back on a later view and are queued again then.
    assert!(matches!(
        covers.looked_up("Artist 49", "Album").await,
        Lookup::Queued
    ));
    assert!(up.searches.load(Ordering::SeqCst) <= 1);
    let _ = std::fs::remove_dir_all(dir);
}

fn used(dir: &Path) -> u64 {
    ["caa", "musicbrainz"]
        .iter()
        .filter_map(|sub| std::fs::read_dir(dir.join(sub)).ok())
        .flatten()
        .flatten()
        .map(|e| e.metadata().unwrap().len())
        .sum()
}

fn age(file: &Path, days: u64) {
    let at = SystemTime::now() - Duration::from_secs(days * 24 * 60 * 60);
    std::fs::File::options()
        .write(true)
        .open(file)
        .unwrap()
        .set_modified(at)
        .unwrap();
}

#[tokio::test]
async fn the_cache_stays_within_its_limit_keeping_what_was_seen_last() {
    let (_, url) = upstream().await;
    let dir = cache_dir();
    // Each cover is the JPEG start and its 36-letter ID: 47 bytes.
    let cover = (JPEG.len() + 36) as u64;
    let covers = Covers::new(Settings {
        cache_max: cover * 10,
        ..settings(&url, &dir)
    });
    for n in 1..=10 {
        covers.front(mbid(n), 250).await.unwrap();
    }
    assert_eq!(used(&dir), cover * 10);
    assert_eq!(covers.cache_used().await, Some(cover * 10));

    // Cover n was last seen 20 − n days ago; cover 1 is seen again now.
    for n in 1..=10 {
        age(&dir.join(format!("caa/{}-250", mbid(n))), 20 - n as u64);
    }
    covers.front(mbid(1), 250).await.unwrap();
    // A left-over half-written file goes first.
    let part = dir.join("caa/.left-over.part");
    std::fs::write(&part, vec![b'x'; 10]).unwrap();
    age(&part, 1);

    // The 11th cover goes over the limit: down to 90 %, the oldest first.
    covers.front(mbid(11), 250).await.unwrap();
    assert!(!part.exists());
    let left = used(&dir);
    assert!(left <= cover * 9, "{left} bytes left");
    assert_eq!(covers.cache_used().await, Some(left));
    let kept = |n: u128| dir.join(format!("caa/{}-250", mbid(n))).exists();
    assert!(kept(1), "the cover seen last went");
    assert!(!kept(2) && !kept(3), "the oldest stayed");
    assert!(kept(10) && kept(11));

    // A cover that went is fetched again when asked for.
    assert!(covers.front(mbid(2), 250).await.is_some());
    assert!(used(&dir) <= cover * 10);
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn a_cache_left_from_before_counts_from_the_start() {
    let (_, url) = upstream().await;
    let dir = cache_dir();
    std::fs::create_dir_all(dir.join("caa")).unwrap();
    let old = dir.join(format!("caa/{}-250", mbid(99)));
    let mut file = std::fs::File::create(&old).unwrap();
    file.write_all(&vec![b'x'; 1000]).unwrap();
    age(&old, 5);
    let covers = Covers::new(Settings {
        cache_max: 500,
        ..settings(&url, &dir)
    });
    covers.front(mbid(1), 250).await.unwrap();
    assert!(!old.exists(), "the old file over the limit stayed");
    assert!(used(&dir) <= 450);
    let _ = std::fs::remove_dir_all(dir);
}
