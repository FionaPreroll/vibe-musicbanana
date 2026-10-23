//! Album covers through the server, against stand-ins for the Cover Art
//! Archive and MusicBrainz.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

use axum::{
    Json, Router,
    body::Body,
    extract::{Path as UrlPath, Query, State},
    http::{Request, StatusCode, header},
    response::{IntoResponse, Redirect, Response},
    routing::get,
};
use http_body_util::BodyExt;
use musicbanana::{
    AppState,
    covers::{Covers, Settings},
    router,
};
use reqwest::Url;
use serde_json::{Value, json};
use sqlx::PgPool;
use tokio::net::TcpListener;
use tower::ServiceExt;
use uuid::Uuid;

// fixtures/profiles.sql: Fiona's default profile (public) has Geräusch by
// Die Ärzte (release 1) and Debut by Björk (release 2); her private profile
// "arbeit" and alex's profile have neither.

/// The release group of Geräusch, made up.
const GERAEUSCH: &str = "5b11f4ce-a62d-471e-81fc-a69a8278c7da";
/// What the MusicBrainz stand-in finds for Björk's Debut.
const DEBUT: &str = "1a2b3c4d-0000-4000-8000-000000000002";
/// A JPEG as far as the server looks.
const JPEG: &[u8] = &[0xff, 0xd8, 0xff, 0xe0, 0, 16, b'J', b'F', b'I', b'F', 0];

#[derive(Default)]
struct Upstream {
    /// Cover files asked for, as `<mbid>/<file>`.
    covers: Mutex<Vec<String>>,
    /// Searches made, as their query.
    searches: Mutex<Vec<String>>,
    /// Release groups the archive has a cover for.
    has: Mutex<Vec<String>>,
    /// Answers 500 to every cover while set.
    down: Mutex<bool>,
    /// Answers with a web page instead of a cover while set.
    junk: Mutex<bool>,
}

async fn archive(
    State(up): State<Arc<Upstream>>,
    UrlPath((mbid, file)): UrlPath<(String, String)>,
) -> Response {
    up.covers.lock().unwrap().push(format!("{mbid}/{file}"));
    if *up.down.lock().unwrap() {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    if *up.junk.lock().unwrap() {
        return ([(header::CONTENT_TYPE, "text/html")], "<html>").into_response();
    }
    if !up.has.lock().unwrap().contains(&mbid) {
        return StatusCode::NOT_FOUND.into_response();
    }
    // As the real archive: off to archive.org, where the file is.
    Redirect::temporary(&format!("/download/{mbid}/{file}.jpg")).into_response()
}

async fn download(UrlPath((_, file)): UrlPath<(String, String)>) -> Response {
    let mut bytes = JPEG.to_vec();
    bytes.extend_from_slice(file.as_bytes());
    ([(header::CONTENT_TYPE, "image/jpeg")], bytes).into_response()
}

async fn search(
    State(up): State<Arc<Upstream>>,
    Query(query): Query<HashMap<String, String>>,
) -> Json<Value> {
    let q = query["query"].clone();
    up.searches.lock().unwrap().push(q.clone());
    if !q.contains("\"Debut\"") {
        return Json(json!({"release-groups": []}));
    }
    Json(json!({
        "release-groups": [
            // Close, but another title: never taken.
            {"id": "1a2b3c4d-0000-4000-8000-000000000001", "score": 100,
             "title": "Debut (Remixes)",
             "artist-credit": [{"name": "Björk", "artist": {"name": "Björk"}}]},
            // Another artist of the same title.
            {"id": "1a2b3c4d-0000-4000-8000-000000000003", "score": 98, "title": "Debut",
             "artist-credit": [{"name": "Someone Else", "artist": {"name": "Someone Else"}}]},
            {"id": DEBUT, "score": 97, "title": "Debut",
             "artist-credit": [{"name": "björk", "joinphrase": "", "artist": {"name": "Björk"}}]}
        ]
    }))
}

async fn upstream() -> (Arc<Upstream>, Url) {
    let up = Arc::new(Upstream::default());
    let app = Router::new()
        .route("/release-group/{mbid}/{file}", get(archive))
        .route("/download/{mbid}/{file}", get(download))
        .route("/ws/2/release-group", get(search))
        .with_state(up.clone());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = Url::parse(&format!("http://{}/", listener.local_addr().unwrap())).unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (up, url)
}

fn cache_dir() -> PathBuf {
    std::env::temp_dir().join(format!("musicbanana-covers-{}", Uuid::new_v4()))
}

fn settings(url: &Url, dir: &Path, lookup: bool) -> Settings {
    Settings {
        enabled: true,
        lookup,
        cache_dir: dir.to_owned(),
        archive: url.clone(),
        musicbrainz: url.clone(),
        search_interval: Duration::ZERO,
    }
}

fn app(db: PgPool, settings: Settings) -> Router {
    let mut state = AppState::new(db);
    state.covers = Arc::new(Covers::new(settings));
    router(state, Path::new("does-not-exist"))
}

struct Reply {
    status: StatusCode,
    headers: axum::http::HeaderMap,
    body: Vec<u8>,
}

async fn get_(app: &Router, uri: &str) -> Reply {
    let res = app
        .clone()
        .oneshot(Request::get(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = res.status();
    let headers = res.headers().clone();
    let body = res.into_body().collect().await.unwrap().to_bytes().to_vec();
    Reply {
        status,
        headers,
        body,
    }
}

const GERAEUSCH_COVER: &str = "/api/profiles/Fiona/default/releases/1/cover";
const DEBUT_COVER: &str = "/api/profiles/Fiona/default/releases/2/cover";

async fn tag_geraeusch(db: &PgPool) {
    sqlx::query("INSERT INTO release_mbid (artist_id, mbid, release_id) VALUES (1, $1, 1)")
        .bind(Uuid::parse_str(GERAEUSCH).unwrap())
        .execute(db)
        .await
        .unwrap();
}

#[sqlx::test(fixtures("profiles"))]
async fn off_by_default(db: PgPool) {
    tag_geraeusch(&db).await;
    let app = router(AppState::new(db), Path::new("does-not-exist"));

    let config = get_(&app, "/api/config").await;
    assert_eq!(config.status, StatusCode::OK);
    let config: Value = serde_json::from_slice(&config.body).unwrap();
    assert_eq!(config, json!({"covers": false}));
    assert_eq!(
        get_(&app, GERAEUSCH_COVER).await.status,
        StatusCode::NOT_FOUND
    );
}

#[sqlx::test(fixtures("profiles"))]
async fn covers_come_once_from_the_archive(db: PgPool) {
    tag_geraeusch(&db).await;
    let (up, url) = upstream().await;
    up.has.lock().unwrap().push(GERAEUSCH.into());
    let dir = cache_dir();
    let app = app(db.clone(), settings(&url, &dir, false));

    let config: Value = serde_json::from_slice(&get_(&app, "/api/config").await.body).unwrap();
    assert_eq!(config, json!({"covers": true}));

    let cover = get_(&app, GERAEUSCH_COVER).await;
    assert_eq!(cover.status, StatusCode::OK);
    assert_eq!(cover.headers[header::CONTENT_TYPE], "image/jpeg");
    assert!(
        cover.headers[header::CACHE_CONTROL]
            .to_str()
            .unwrap()
            .starts_with("private")
    );
    assert!(cover.body.starts_with(JPEG) && cover.body.ends_with(b"front-250.jpg"));

    // From the cache now, also with another server started on the same directory.
    let again = get_(&app, GERAEUSCH_COVER).await;
    assert_eq!(again.body, cover.body);
    let restarted = self::app(db.clone(), settings(&url, &dir, false));
    assert_eq!(get_(&restarted, GERAEUSCH_COVER).await.body, cover.body);
    assert_eq!(
        *up.covers.lock().unwrap(),
        [format!("{GERAEUSCH}/front-250")]
    );
    assert!(dir.join(format!("caa/{GERAEUSCH}-250")).exists());

    // The album page asks for the larger one.
    let large = get_(&app, &format!("{GERAEUSCH_COVER}?size=500")).await;
    assert_eq!(large.status, StatusCode::OK);
    assert!(large.body.ends_with(b"front-500.jpg"));
    assert_eq!(
        get_(&app, &format!("{GERAEUSCH_COVER}?size=1200"))
            .await
            .status,
        StatusCode::BAD_REQUEST
    );

    // Without an ID and without searches, Debut has none, and nobody is asked.
    let debut = get_(&app, DEBUT_COVER).await;
    assert_eq!(debut.status, StatusCode::NOT_FOUND);
    assert_eq!(up.covers.lock().unwrap().len(), 2);
    assert!(up.searches.lock().unwrap().is_empty());

    let _ = std::fs::remove_dir_all(dir);
}

#[sqlx::test(fixtures("profiles"))]
async fn only_albums_the_profile_has(db: PgPool) {
    tag_geraeusch(&db).await;
    let (up, url) = upstream().await;
    up.has.lock().unwrap().push(GERAEUSCH.into());
    let dir = cache_dir();
    let app = app(db.clone(), settings(&url, &dir, false));

    // alex's profile has no listen of Geräusch's.
    for uri in [
        "/api/profiles/alex/default/releases/1/cover",
        // Private, and not of the viewer (nobody is logged in).
        "/api/profiles/Fiona/arbeit/releases/1/cover",
        "/api/profiles/Fiona/default/releases/99/cover",
    ] {
        assert_eq!(get_(&app, uri).await.status, StatusCode::NOT_FOUND, "{uri}");
    }
    assert!(up.covers.lock().unwrap().is_empty());

    // A merged album shows the cover of the one it went into.
    sqlx::query(
        "INSERT INTO release (id, title, artist_id, merged_into) OVERRIDING SYSTEM VALUE
         VALUES (7, 'Geraeusch', 1, 1)",
    )
    .execute(&db)
    .await
    .unwrap();
    let merged = get_(&app, "/api/profiles/Fiona/default/releases/7/cover").await;
    assert_eq!(merged.status, StatusCode::OK);

    let _ = std::fs::remove_dir_all(dir);
}

#[sqlx::test(fixtures("profiles"))]
async fn missing_covers_are_remembered_failures_not(db: PgPool) {
    tag_geraeusch(&db).await;
    let (up, url) = upstream().await;
    let dir = cache_dir();
    let app = app(db.clone(), settings(&url, &dir, false));

    // The archive is down, or sends something else than a picture: no cover,
    // and asked again next time.
    *up.junk.lock().unwrap() = true;
    assert_eq!(
        get_(&app, GERAEUSCH_COVER).await.status,
        StatusCode::NOT_FOUND
    );
    *up.junk.lock().unwrap() = false;
    *up.down.lock().unwrap() = true;
    assert_eq!(
        get_(&app, GERAEUSCH_COVER).await.status,
        StatusCode::NOT_FOUND
    );
    *up.down.lock().unwrap() = false;
    // Up again, without a cover for it: remembered.
    assert_eq!(
        get_(&app, GERAEUSCH_COVER).await.status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        get_(&app, GERAEUSCH_COVER).await.status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(up.covers.lock().unwrap().len(), 3);
    assert!(dir.join(format!("caa/{GERAEUSCH}-250.missing")).exists());

    // Not served: whatever is in the cache that is not an image.
    std::fs::write(dir.join(format!("caa/{GERAEUSCH}-500")), b"<html>").unwrap();
    up.has.lock().unwrap().push(GERAEUSCH.into());
    let large = get_(&app, &format!("{GERAEUSCH_COVER}?size=500")).await;
    assert!(large.body.starts_with(JPEG));

    let _ = std::fs::remove_dir_all(dir);
}

#[sqlx::test(fixtures("profiles"))]
async fn albums_without_an_id_are_looked_up_in_the_background(db: PgPool) {
    let (up, url) = upstream().await;
    up.has.lock().unwrap().push(DEBUT.into());
    let dir = cache_dir();
    let app = app(db.clone(), settings(&url, &dir, true));

    // The first view queues the search and gets no cover, nor keeps that answer.
    let first = get_(&app, DEBUT_COVER).await;
    assert_eq!(first.status, StatusCode::NOT_FOUND);
    assert_eq!(first.headers[header::CACHE_CONTROL], "no-store");

    let mut cover = None;
    for _ in 0..100 {
        tokio::time::sleep(Duration::from_millis(20)).await;
        let reply = get_(&app, DEBUT_COVER).await;
        if reply.status == StatusCode::OK {
            cover = Some(reply);
            break;
        }
    }
    let cover = cover.expect("the cover after the search");
    assert!(cover.body.starts_with(JPEG));
    assert_eq!(*up.covers.lock().unwrap(), [format!("{DEBUT}/front-250")]);
    let searches = up.searches.lock().unwrap().clone();
    assert_eq!(searches, [r#"releasegroup:"Debut" AND artist:"Björk""#]);

    // Geräusch has no ID either and MusicBrainz finds nothing: remembered too.
    let geraeusch = get_(&app, GERAEUSCH_COVER).await;
    assert_eq!(geraeusch.status, StatusCode::NOT_FOUND);
    for _ in 0..100 {
        tokio::time::sleep(Duration::from_millis(20)).await;
        if up.searches.lock().unwrap().len() == 2 {
            break;
        }
    }
    tokio::time::sleep(Duration::from_millis(50)).await;
    let later = get_(&app, GERAEUSCH_COVER).await;
    assert_eq!(later.status, StatusCode::NOT_FOUND);
    assert_eq!(
        later.headers[header::CACHE_CONTROL],
        "private, max-age=3600"
    );
    assert_eq!(up.searches.lock().unwrap().len(), 2);

    let _ = std::fs::remove_dir_all(dir);
}
