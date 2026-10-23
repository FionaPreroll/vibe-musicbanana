//! Album covers from the Cover Art Archive, served by musicbanana itself.
//!
//! The browser never talks to archive.org: it asks
//! `GET /api/profiles/{username}/{slug}/releases/{id}/cover`, and the server
//! fetches the front cover of the album's release group once and keeps it in a
//! directory on disk (`COVER_ART_CACHE`). Albums without a MusicBrainz ID can be
//! looked up by artist and title at MusicBrainz (`COVER_ART_LOOKUP`); that runs in
//! the background, one search a second as MusicBrainz asks, so the first page
//! with many such albums shows placeholders and a later one the covers.
//!
//! Both are off unless switched on: nothing leaves the server by default.
//!
//! The cache directory holds
//! - `caa/<mbid>-<size>`: a cover as the archive sent it,
//! - `caa/<mbid>-<size>.missing`: the archive has no cover for it,
//! - `musicbrainz/<hash>.json`: what a search by artist and title found.
//!
//! "No cover" and "nothing found" are asked again after [`RETRY_AFTER`]. A
//! failed request (archive.org down, too slow) is not kept, so the next view
//! tries again. Deleting the directory forgets everything; it fills up again.
//!
//! What guards the requests to the outside:
//! - The addresses are built from fixed bases and an ID out of the database;
//!   nothing a visitor sends goes into them but the size, 250 or 500.
//! - The archive sends covers on to archive.org. Redirects are followed only to
//!   the archive itself and over HTTPS to the hosts in [`Settings::redirect_hosts`]
//!   (archive.org and its subdomains); any other is refused, logged without
//!   its query and with control characters escaped (see [`loggable`]), and the
//!   cover counts as missing. MusicBrainz is not followed anywhere.
//! - Answers are read in pieces and dropped once larger than [`MAX_BYTES`] (a
//!   cover) or [`MAX_SEARCH_BYTES`] (a search), whatever they claim to be.
//! - Only JPEG, PNG, GIF and WebP are served, recognized by their first bytes.
//! - At most 4 covers are fetched at once, each within 15 seconds, and at
//!   most [`Settings::search_queue`] searches wait.
//! - The cache stays below `COVER_ART_CACHE_MAX`; past it, the covers seen
//!   longest ago go first.

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
    time::{Duration, Instant, SystemTime},
};

use anyhow::{Context, bail};
use axum::{
    Router,
    body::Bytes,
    extract::{Path as UrlPath, Query, State},
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::sync::{Semaphore, mpsc};
use unicode_normalization::UnicodeNormalization;
use uuid::Uuid;

use crate::{AppError, AppState, account::Viewer, profiles::find_profile};

/// How long "the archive has no cover" and "MusicBrainz found nothing" hold.
pub const RETRY_AFTER: Duration = Duration::from_secs(30 * 24 * 60 * 60);

/// The sizes the frontend asks for: list thumbnails and the album page. The
/// archive has these as `front-250` and `front-500`.
const SIZES: [u16; 2] = [250, 500];

/// Larger answers are not taken for a cover.
pub const MAX_BYTES: usize = 5 * 1024 * 1024;

/// Larger answers are not taken for a search at MusicBrainz.
pub const MAX_SEARCH_BYTES: usize = 1024 * 1024;

/// The cache limit when `COVER_ART_CACHE_MAX` is not set.
pub const DEFAULT_CACHE_MAX: u64 = 1024 * 1024 * 1024;

/// `COVER_ART_CACHE_MAX` takes no less, so that a few covers always fit.
pub const MIN_CACHE_MAX: u64 = 10 * 1024 * 1024;

/// Once over its limit, the cache is cleared down to this share of it, so
/// that it is not cleared again with every new cover.
const CACHE_LOW_WATER: f64 = 0.9;

/// Left-over half-written files older than this go at the next clearing.
const STALE_PART: Duration = Duration::from_secs(60 * 60);

/// A logged address is cut to this many characters.
const LOGGED_URL_CHARS: usize = 200;

const USER_AGENT: &str = concat!(
    "musicbanana/",
    env!("CARGO_PKG_VERSION"),
    " ( https://github.com/FionaPreroll/vibe-musicbanana )"
);

pub fn routes() -> Router<AppState> {
    Router::new().route("/config", get(config)).route(
        "/profiles/{username}/{slug}/releases/{id}/cover",
        get(cover),
    )
}

/// What the server offers, for the frontend: whether to show covers at all.
#[derive(Serialize)]
struct Config {
    covers: bool,
}

async fn config(State(state): State<AppState>) -> impl IntoResponse {
    axum::Json(Config {
        covers: state.covers.enabled(),
    })
}

/// How covers are fetched; see the module documentation.
#[derive(Clone, Debug)]
pub struct Settings {
    /// Fetch covers from the Cover Art Archive (`COVER_ART`).
    pub enabled: bool,
    /// Search MusicBrainz for albums without an ID (`COVER_ART_LOOKUP`).
    pub lookup: bool,
    /// Where covers are kept (`COVER_ART_CACHE`).
    pub cache_dir: PathBuf,
    /// The most the cache directory may hold, in bytes (`COVER_ART_CACHE_MAX`).
    pub cache_max: u64,
    /// The Cover Art Archive; tests point it at a stand-in.
    pub archive: Url,
    /// Hosts the archive may send on to over HTTPS, each with its subdomains.
    pub redirect_hosts: Vec<String>,
    /// MusicBrainz's web service.
    pub musicbrainz: Url,
    /// The time between two searches at MusicBrainz, which allows one a second.
    pub search_interval: Duration,
    /// At most this many searches wait; more are not queued and asked again
    /// on a later view.
    pub search_queue: usize,
}

impl Settings {
    /// Off, with the real archive and MusicBrainz once switched on.
    pub fn off(cache_dir: impl Into<PathBuf>) -> Self {
        Self {
            enabled: false,
            lookup: false,
            cache_dir: cache_dir.into(),
            cache_max: DEFAULT_CACHE_MAX,
            archive: Url::parse("https://coverartarchive.org/").unwrap(),
            redirect_hosts: vec!["archive.org".into()],
            musicbrainz: Url::parse("https://musicbrainz.org/").unwrap(),
            search_interval: Duration::from_millis(1100),
            search_queue: 1000,
        }
    }

    /// The archive and MusicBrainz both at `url`, for tests with a stand-in.
    pub fn with_upstream(mut self, url: &Url) -> Self {
        self.archive = url.clone();
        self.musicbrainz = url.clone();
        self
    }

    /// From `COVER_ART`, `COVER_ART_LOOKUP`, `COVER_ART_CACHE` and
    /// `COVER_ART_CACHE_MAX`.
    pub fn from_env(get: impl Fn(&str) -> Option<String>) -> anyhow::Result<Self> {
        let switch = |name: &str| match get(name).as_deref().map(str::trim) {
            None | Some("" | "off" | "false" | "0") => Ok(false),
            Some("on" | "true" | "1") => Ok(true),
            Some(other) => bail!("{name} is on or off, not {other}"),
        };
        let mut settings = Self::off(
            get("COVER_ART_CACHE")
                .filter(|dir| !dir.trim().is_empty())
                .unwrap_or_else(|| "cache/covers".into()),
        );
        settings.enabled = switch("COVER_ART")?;
        settings.lookup = switch("COVER_ART_LOOKUP")?;
        if settings.lookup && !settings.enabled {
            bail!("COVER_ART_LOOKUP needs COVER_ART=on");
        }
        if let Some(max) = get("COVER_ART_CACHE_MAX").filter(|max| !max.trim().is_empty()) {
            settings.cache_max = parse_size(&max).with_context(|| {
                format!("COVER_ART_CACHE_MAX is a size such as 500M or 2G, not {max}")
            })?;
            if settings.cache_max < MIN_CACHE_MAX {
                bail!("COVER_ART_CACHE_MAX is at least 10M");
            }
        }
        Ok(settings)
    }
}

/// A size such as `1073741824`, `500M`, `500 MB`, `2G` or `2GiB`; K, M, G
/// and T count in steps of 1024.
pub fn parse_size(text: &str) -> anyhow::Result<u64> {
    let text = text.trim();
    let split = text
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(text.len());
    let (number, unit) = text.split_at(split);
    let number: u64 = number.parse().context("no number")?;
    let shift = match unit.trim().to_ascii_uppercase().as_str() {
        "" | "B" => 0,
        "K" | "KB" | "KIB" => 10,
        "M" | "MB" | "MIB" => 20,
        "G" | "GB" | "GIB" => 30,
        "T" | "TB" | "TIB" => 40,
        other => bail!("unknown unit {other}"),
    };
    number.checked_mul(1 << shift).context("too large a number")
}

/// Whether the archive may send a request on to `url`: back to the archive
/// itself (same scheme, host and port), or over HTTPS on the default port to
/// one of `hosts` or a subdomain of it. Addresses with a user name or password
/// never pass.
pub fn redirect_allowed(url: &Url, archive: &Url, hosts: &[String]) -> bool {
    if !url.username().is_empty() || url.password().is_some() {
        return false;
    }
    let Some(host) = url.host_str() else {
        return false;
    };
    if url.scheme() == archive.scheme()
        && Some(host) == archive.host_str()
        && url.port_or_known_default() == archive.port_or_known_default()
    {
        return true;
    }
    url.scheme() == "https"
        && url.port().is_none()
        && !host.starts_with('[')
        && host.parse::<std::net::Ipv4Addr>().is_err()
        && hosts.iter().any(|allowed| {
            let allowed = allowed.trim_matches('.').to_ascii_lowercase();
            !allowed.is_empty()
                && (host == allowed
                    || host
                        .strip_suffix(allowed.as_str())
                        .is_some_and(|sub| sub.len() > 1 && sub.ends_with('.')))
        })
}

/// An address as it may go into the log: scheme, host, port and path only (a
/// query or a user name could hold a secret), with everything outside printable
/// ASCII escaped so that it cannot forge or break log lines, and cut to
/// [`LOGGED_URL_CHARS`].
pub fn loggable(url: &Url) -> String {
    let mut shown = format!("{}://{}", url.scheme(), url.host_str().unwrap_or("?"));
    if let Some(port) = url.port() {
        shown.push_str(&format!(":{port}"));
    }
    shown.push_str(url.path());
    let mut safe = String::new();
    for c in shown.chars() {
        if c.is_ascii_graphic() {
            safe.push(c);
        } else {
            safe.extend(c.escape_default());
        }
    }
    if safe.chars().count() > LOGGED_URL_CHARS {
        safe = safe.chars().take(LOGGED_URL_CHARS).collect();
        safe.push('…');
    }
    safe
}

/// Fetches and keeps covers; shared by all requests.
pub struct Covers {
    settings: Settings,
    archive_http: OnceLock<reqwest::Client>,
    search_http: OnceLock<reqwest::Client>,
    /// At most this many requests to the archive at once.
    fetching: Semaphore,
    /// Albums waiting for a search at MusicBrainz, by cache key.
    queued: Arc<Mutex<HashSet<String>>>,
    searches: OnceLock<mpsc::Sender<Search>>,
    cache: Arc<Cache>,
}

/// The cache directory and its limit.
struct Cache {
    dir: PathBuf,
    max: u64,
    /// How much it holds, once counted.
    used: tokio::sync::Mutex<Option<u64>>,
}

impl Cache {
    /// Writes a file into the cache and keeps the cache within its limit.
    async fn store(&self, path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
        write(path, bytes).await?;
        let mut used = self.used.lock().await;
        let total = match *used {
            Some(total) => total + bytes.len() as u64,
            None => count(&self.dir).await,
        };
        *used = Some(if total > self.max {
            shrink(&self.dir, self.max).await
        } else {
            total
        });
        Ok(())
    }

    fn search_file(&self, key: &str) -> PathBuf {
        self.dir.join("musicbrainz").join(format!("{key}.json"))
    }
}

/// What [`Covers::looked_up`] knows about an album without an ID.
pub enum Lookup {
    /// Searched: the release group found, if any.
    Done(Option<Uuid>),
    /// The search is still to come.
    Queued,
}

/// An album to look up at MusicBrainz.
struct Search {
    key: String,
    artist: String,
    title: String,
}

/// What a search found, as kept on disk.
#[derive(Serialize, Deserialize)]
struct Found {
    mbid: Option<Uuid>,
}

/// A cover with its media type.
pub struct Image {
    pub bytes: Bytes,
    pub content_type: &'static str,
}

/// Why a fetch gave nothing.
enum Failure {
    /// The archive sent the request somewhere it may not go.
    Refused,
    Other(anyhow::Error),
}

impl<E: Into<anyhow::Error>> From<E> for Failure {
    fn from(err: E) -> Self {
        Self::Other(err.into())
    }
}

impl Default for Covers {
    fn default() -> Self {
        Self::new(Settings::off("cache/covers"))
    }
}

impl Covers {
    pub fn new(settings: Settings) -> Self {
        Self {
            archive_http: OnceLock::new(),
            search_http: OnceLock::new(),
            fetching: Semaphore::new(4),
            queued: Arc::default(),
            searches: OnceLock::new(),
            cache: Arc::new(Cache {
                dir: settings.cache_dir.clone(),
                max: settings.cache_max,
                used: tokio::sync::Mutex::new(None),
            }),
            settings,
        }
    }

    pub fn enabled(&self) -> bool {
        self.settings.enabled
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// How many searches are waiting or running.
    pub fn searches_pending(&self) -> usize {
        self.queued.lock().unwrap().len()
    }

    fn client(policy: reqwest::redirect::Policy) -> reqwest::Client {
        // As in yourspotify.rs: reqwest wants the crypto provider set up front.
        let _ = rustls::crypto::ring::default_provider().install_default();
        reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .user_agent(USER_AGENT)
            .redirect(policy)
            .build()
            .expect("an HTTP client")
    }

    /// For the archive: follows its redirects only where [`redirect_allowed`]
    /// lets it, at most 5, and logs the others.
    fn archive_http(&self) -> &reqwest::Client {
        self.archive_http.get_or_init(|| {
            let archive = self.settings.archive.clone();
            let hosts = self.settings.redirect_hosts.clone();
            Self::client(reqwest::redirect::Policy::custom(move |attempt| {
                if attempt.previous().len() > 5 {
                    tracing::warn!(
                        target: "musicbanana::covers",
                        "refused redirect to {}: too many redirects",
                        loggable(attempt.url())
                    );
                    attempt.error(RefusedRedirect)
                } else if redirect_allowed(attempt.url(), &archive, &hosts) {
                    attempt.follow()
                } else {
                    let from = attempt.previous().last().map_or_else(String::new, loggable);
                    tracing::warn!(
                        target: "musicbanana::covers",
                        "refused redirect from {from} to {}: not an allowed host",
                        loggable(attempt.url())
                    );
                    attempt.error(RefusedRedirect)
                }
            }))
        })
    }

    /// For MusicBrainz, which has no reason to send a search elsewhere.
    fn search_http(&self) -> &reqwest::Client {
        self.search_http
            .get_or_init(|| Self::client(reqwest::redirect::Policy::none()))
    }

    /// The cover of a release group, from the cache or the archive. `None`
    /// when the archive has none, or could not be asked just now.
    pub async fn front(&self, mbid: Uuid, size: u16) -> Option<Image> {
        let dir = self.settings.cache_dir.join("caa");
        let file = dir.join(format!("{mbid}-{size}"));
        let missing = dir.join(format!("{mbid}-{size}.missing"));
        if let Ok(bytes) = tokio::fs::read(&file).await
            && let Some(content_type) = image_type(&bytes)
        {
            seen(&file).await;
            return Some(Image {
                bytes: bytes.into(),
                content_type,
            });
        }
        if fresh(&missing).await {
            return None;
        }
        let _permit = self.fetching.acquire().await.ok()?;
        match self.download(mbid, size).await {
            Ok(Some(image)) => {
                if let Err(err) = self.cache.store(&file, &image.bytes).await {
                    tracing::warn!("keeping the cover of {mbid}: {err:#}");
                }
                let _ = tokio::fs::remove_file(&missing).await;
                Some(image)
            }
            // A refused redirect is not tried again with every view either.
            Ok(None) | Err(Failure::Refused) => {
                if let Err(err) = self.cache.store(&missing, b"").await {
                    tracing::warn!("noting the missing cover of {mbid}: {err:#}");
                }
                None
            }
            Err(Failure::Other(err)) => {
                tracing::warn!("fetching the cover of {mbid}: {err:#}");
                None
            }
        }
    }

    async fn download(&self, mbid: Uuid, size: u16) -> Result<Option<Image>, Failure> {
        let url = self
            .settings
            .archive
            .join(&format!("release-group/{mbid}/front-{size}"))?;
        let response = match self.archive_http().get(url).send().await {
            Ok(response) => response,
            Err(err) if is_refused(&err) => return Err(Failure::Refused),
            Err(err) => {
                return Err(anyhow::Error::new(err.without_url())
                    .context("asking the Cover Art Archive")
                    .into());
            }
        };
        match response.status() {
            StatusCode::OK => {}
            // No release of the group has a front cover, or MusicBrainz does not
            // know the ID (any more).
            StatusCode::NOT_FOUND | StatusCode::BAD_REQUEST => return Ok(None),
            status => return Err(anyhow::anyhow!("the Cover Art Archive answered {status}").into()),
        }
        let bytes = read_limited(response, MAX_BYTES)
            .await
            .context("reading the cover")?;
        let content_type = image_type(&bytes).context("the archive sent no image")?;
        Ok(Some(Image {
            bytes,
            content_type,
        }))
    }

    /// The release group a search by artist and title found, if one was made.
    /// Otherwise the search is queued, to ask again later.
    pub async fn looked_up(&self, artist: &str, title: &str) -> Lookup {
        let key = search_key(artist, title);
        let file = self.cache.search_file(&key);
        if let Ok(text) = tokio::fs::read(&file).await
            && let Ok(found) = serde_json::from_slice::<Found>(&text)
            && (found.mbid.is_some() || fresh(&file).await)
        {
            return Lookup::Done(found.mbid);
        }
        self.queue(Search {
            key,
            artist: artist.to_owned(),
            title: title.to_owned(),
        });
        Lookup::Queued
    }

    fn queue(&self, search: Search) {
        if !self.queued.lock().unwrap().insert(search.key.clone()) {
            return;
        }
        let sender = self.searches.get_or_init(|| {
            let (sender, receiver) = mpsc::channel(self.settings.search_queue.max(1));
            tokio::spawn(search_worker(
                receiver,
                Worker {
                    settings: self.settings.clone(),
                    http: self.search_http().clone(),
                    queued: self.queued.clone(),
                    cache: self.cache.clone(),
                },
            ));
            sender
        });
        let key = search.key.clone();
        if sender.try_send(search).is_err() {
            // Full: a later view asks again.
            self.queued.lock().unwrap().remove(&key);
            tracing::debug!(target: "musicbanana::covers", "MusicBrainz search queue full");
        }
    }

    /// What the cache directory holds, as last counted (for tests and logs).
    pub async fn cache_used(&self) -> Option<u64> {
        *self.cache.used.lock().await
    }
}

/// The error a refused redirect ends the request with.
#[derive(Debug)]
struct RefusedRedirect;

impl std::fmt::Display for RefusedRedirect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("redirect refused")
    }
}

impl std::error::Error for RefusedRedirect {}

fn is_refused(err: &reqwest::Error) -> bool {
    let mut source: Option<&dyn std::error::Error> = Some(err);
    while let Some(err) = source {
        if err.is::<RefusedRedirect>() {
            return true;
        }
        source = err.source();
    }
    false
}

/// Reads an answer in pieces, giving up once it grows past `limit`, also when
/// it claimed a smaller size or none.
async fn read_limited(mut response: reqwest::Response, limit: usize) -> anyhow::Result<Bytes> {
    if response.content_length().is_some_and(|n| n > limit as u64) {
        bail!("the answer is larger than {limit} bytes");
    }
    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(reqwest::Error::without_url)?
    {
        if body.len() + chunk.len() > limit {
            bail!("the answer is larger than {limit} bytes");
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body.into())
}

/// The cache's files: path, size and last change, the half-written ones too.
async fn files(dir: &Path) -> Vec<(PathBuf, u64, SystemTime)> {
    let dir = dir.to_owned();
    tokio::task::spawn_blocking(move || {
        let mut found = Vec::new();
        for sub in ["caa", "musicbrainz"] {
            let Ok(entries) = std::fs::read_dir(dir.join(sub)) else {
                continue;
            };
            for entry in entries.flatten() {
                if let Ok(meta) = entry.metadata()
                    && meta.is_file()
                {
                    let changed = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
                    found.push((entry.path(), meta.len(), changed));
                }
            }
        }
        found
    })
    .await
    .unwrap_or_default()
}

async fn count(dir: &Path) -> u64 {
    files(dir).await.iter().map(|(_, size, _)| size).sum()
}

/// Deletes the files seen longest ago until the cache is down to
/// [`CACHE_LOW_WATER`] of `max`; returns what it holds then.
async fn shrink(dir: &Path, max: u64) -> u64 {
    let mut files = files(dir).await;
    let mut total: u64 = files.iter().map(|(_, size, _)| size).sum();
    let target = (max as f64 * CACHE_LOW_WATER) as u64;
    let now = SystemTime::now();
    // Left-overs of a crash first, then the oldest.
    files.sort_by_key(|(path, _, changed)| {
        let part = path
            .file_name()
            .is_some_and(|n| n.to_string_lossy().ends_with(".part"));
        let stale = now.duration_since(*changed).unwrap_or_default() > STALE_PART;
        (!(part && stale), *changed)
    });
    let mut removed = 0;
    for (path, size, _) in files {
        if total <= target {
            break;
        }
        if tokio::fs::remove_file(&path).await.is_ok() {
            total -= size;
            removed += 1;
        }
    }
    tracing::info!(
        target: "musicbanana::covers",
        "cover cache over its limit: removed {removed} files, {total} bytes left"
    );
    total
}

/// Marks a cover as seen now, so that a full cache keeps it longest. At most
/// once a day, to spare the disk.
async fn seen(file: &Path) {
    let file = file.to_owned();
    let _ = tokio::task::spawn_blocking(move || {
        let now = SystemTime::now();
        let old = std::fs::metadata(&file)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|at| now.duration_since(at).ok())
            .is_some_and(|age| age > Duration::from_secs(24 * 60 * 60));
        if old && let Ok(f) = std::fs::File::options().write(true).open(&file) {
            let _ = f.set_modified(now);
        }
    })
    .await;
}

/// What the background search needs.
struct Worker {
    settings: Settings,
    http: reqwest::Client,
    queued: Arc<Mutex<HashSet<String>>>,
    cache: Arc<Cache>,
}

/// Searches MusicBrainz one album at a time, keeping its interval.
async fn search_worker(mut searches: mpsc::Receiver<Search>, worker: Worker) {
    let Worker {
        settings,
        http,
        queued,
        cache,
    } = worker;
    let mut last: Option<Instant> = None;
    while let Some(search) = searches.recv().await {
        if let Some(last) = last {
            tokio::time::sleep_until((last + settings.search_interval).into()).await;
        }
        last = Some(Instant::now());
        let file = cache.search_file(&search.key);
        match find_release_group(&http, &settings.musicbrainz, &search.artist, &search.title).await
        {
            Ok(mbid) => {
                let text = serde_json::to_vec(&Found { mbid }).unwrap();
                if let Err(err) = cache.store(&file, &text).await {
                    tracing::warn!("keeping a MusicBrainz search: {err:#}");
                }
            }
            Err(err) => tracing::warn!(
                "searching MusicBrainz for {:?} – {:?}: {err:#}",
                search.artist,
                search.title
            ),
        }
        queued.lock().unwrap().remove(&search.key);
    }
}

/// The release group MusicBrainz has under this artist and title, if exactly
/// that: a close match with another title or artist would show a wrong cover.
pub async fn find_release_group(
    http: &reqwest::Client,
    musicbrainz: &Url,
    artist: &str,
    title: &str,
) -> anyhow::Result<Option<Uuid>> {
    let query = format!(
        "releasegroup:\"{}\" AND artist:\"{}\"",
        lucene_phrase(title),
        lucene_phrase(artist)
    );
    let response = http
        .get(musicbrainz.join("ws/2/release-group")?)
        .query(&[("query", query.as_str()), ("fmt", "json"), ("limit", "10")])
        .send()
        .await
        .map_err(reqwest::Error::without_url)
        .context("asking MusicBrainz")?;
    if response.status() != StatusCode::OK {
        bail!("MusicBrainz answered {}", response.status());
    }
    let body = read_limited(response, MAX_SEARCH_BYTES)
        .await
        .context("reading MusicBrainz's answer")?;
    let found: SearchResult =
        serde_json::from_slice(&body).context("reading MusicBrainz's answer")?;
    Ok(best_match(found, artist, title))
}

/// The first result with exactly this title and artist (in one of the
/// spellings MusicBrainz credits), scored 90 or more.
fn best_match(found: SearchResult, artist: &str, title: &str) -> Option<Uuid> {
    let (artist, title) = (normalize(artist), normalize(title));
    found
        .release_groups
        .into_iter()
        .filter(|group| group.score >= 90 && normalize(&group.title) == title)
        .find(|group| {
            let credit: String = group
                .artist_credit
                .iter()
                .map(|c| format!("{}{}", c.name, c.joinphrase))
                .collect();
            normalize(&credit) == artist
                || group
                    .artist_credit
                    .iter()
                    .any(|c| normalize(&c.name) == artist || normalize(&c.artist.name) == artist)
        })
        .map(|group| group.id)
}

#[derive(Deserialize)]
struct SearchResult {
    #[serde(rename = "release-groups", default)]
    release_groups: Vec<ReleaseGroup>,
}

#[derive(Deserialize)]
struct ReleaseGroup {
    id: Uuid,
    #[serde(default)]
    score: u32,
    title: String,
    #[serde(rename = "artist-credit", default)]
    artist_credit: Vec<Credit>,
}

#[derive(Deserialize)]
struct Credit {
    name: String,
    #[serde(default)]
    joinphrase: String,
    artist: CreditedArtist,
}

#[derive(Deserialize)]
struct CreditedArtist {
    name: String,
}

/// Inside quotes, Lucene only needs `"` and `\` escaped.
fn lucene_phrase(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

/// For comparing names: Unicode-normalized, lower case, single spaces, and the
/// common stand-ins for apostrophes and dashes made alike.
fn normalize(text: &str) -> String {
    let text: String = text
        .nfkc()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            '’' | '‘' | '`' | '´' => '\'',
            '‐' | '‑' | '‒' | '–' | '—' => '-',
            c => c,
        })
        .collect();
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The file name of a search; the same album under another spelling of the
/// same letters shares it.
fn search_key(artist: &str, title: &str) -> String {
    let mut hash = Sha256::new();
    hash.update(normalize(artist));
    hash.update([0]);
    hash.update(normalize(title));
    hash.finalize()
        .iter()
        .take(16)
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// The media type of a JPEG, PNG, GIF or WebP image; anything else is not served.
pub fn image_type(bytes: &[u8]) -> Option<&'static str> {
    match bytes {
        [0xff, 0xd8, 0xff, ..] => Some("image/jpeg"),
        [0x89, b'P', b'N', b'G', ..] => Some("image/png"),
        [b'G', b'I', b'F', b'8', ..] => Some("image/gif"),
        [
            b'R',
            b'I',
            b'F',
            b'F',
            _,
            _,
            _,
            _,
            b'W',
            b'E',
            b'B',
            b'P',
            ..,
        ] => Some("image/webp"),
        _ => None,
    }
}

/// Whether the file exists and is younger than [`RETRY_AFTER`].
async fn fresh(path: &Path) -> bool {
    tokio::fs::metadata(path)
        .await
        .and_then(|m| m.modified())
        .ok()
        .and_then(|at| SystemTime::now().duration_since(at).ok())
        .is_some_and(|age| age < RETRY_AFTER)
}

/// Writes through a file of its own and a rename, so a reader never sees half
/// a cover, also when two requests fetch the same one.
async fn write(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    let dir = path.parent().context("no directory")?;
    tokio::fs::create_dir_all(dir)
        .await
        .with_context(|| format!("creating {}", dir.display()))?;
    let part = dir.join(format!(".{}.part", Uuid::new_v4()));
    tokio::fs::write(&part, bytes)
        .await
        .with_context(|| format!("writing {}", part.display()))?;
    tokio::fs::rename(&part, path)
        .await
        .with_context(|| format!("renaming to {}", path.display()))?;
    Ok(())
}

#[derive(Deserialize)]
struct CoverParams {
    /// 250 (default) or 500 pixels wide.
    size: Option<u16>,
}

/// The front cover of an album the profile has listens of; 404 when covers are
/// off, the album has none, or the archive could not be asked just now.
async fn cover(
    State(state): State<AppState>,
    viewer: Viewer,
    UrlPath((username, slug, id)): UrlPath<(String, String, i64)>,
    Query(params): Query<CoverParams>,
) -> Result<Response, AppError> {
    let size = params.size.unwrap_or(250);
    if !SIZES.contains(&size) {
        return Err(AppError::BadRequest("size is 250 or 500".into()));
    }
    let covers = &state.covers;
    if !covers.enabled() {
        return Err(AppError::NotFound);
    }
    let profile = find_profile(&state.db, viewer, &username, &slug).await?;
    // Only albums of this profile, so a private profile's albums stay hidden
    // also as pictures. A merged album shows the one it went into.
    let release = sqlx::query!(
        r#"SELECT r.title, a.name AS artist,
                  array(SELECT DISTINCT m.mbid FROM release_mbid m
                         WHERE m.release_id = r.id ORDER BY m.mbid) AS "mbids!"
             FROM release r
             JOIN artist a ON a.id = r.artist_id
            WHERE r.id = (SELECT coalesce(merged_into, id) FROM release WHERE id = $1)
              AND EXISTS (SELECT FROM listen l
                           WHERE l.release_id = r.id AND l.profile_id = $2)"#,
        id,
        profile.id,
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    let mut mbids = release.mbids;
    let mut later = false;
    if mbids.is_empty() && covers.settings().lookup {
        match covers.looked_up(&release.artist, &release.title).await {
            Lookup::Done(found) => mbids.extend(found),
            Lookup::Queued => later = true,
        }
    }
    for mbid in mbids {
        if let Some(image) = covers.front(mbid, size).await {
            let mut response = (
                [
                    (header::CONTENT_TYPE, image.content_type),
                    // A week in the browser; covers hardly change.
                    (header::CACHE_CONTROL, "private, max-age=604800"),
                ],
                image.bytes,
            )
                .into_response();
            response.headers_mut().insert(
                "x-content-type-options",
                HeaderValue::from_static("nosniff"),
            );
            return Ok(response);
        }
    }
    // While a search is queued, the browser should ask again on the next view.
    let cache = if later {
        "no-store"
    } else {
        "private, max-age=3600"
    };
    Ok((StatusCode::NOT_FOUND, [(header::CACHE_CONTROL, cache)]).into_response())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_compare_loosely() {
        assert_eq!(normalize("Die  Ärzte"), normalize("die ärzte"));
        assert_eq!(normalize("Don’t Stop"), normalize("don't stop"));
        assert_ne!(normalize("Debut"), normalize("Début"));
        assert_eq!(search_key("Björk", "Debut"), search_key("björk", " Debut "));
        assert_ne!(search_key("Björk", "Debut"), search_key("Björk", "Post"));
    }

    #[test]
    fn quotes_are_escaped() {
        assert_eq!(lucene_phrase(r#"a "b" \c"#), r#"a \"b\" \\c"#);
    }

    #[test]
    fn only_images_are_served() {
        assert_eq!(image_type(&[0xff, 0xd8, 0xff, 0xe0]), Some("image/jpeg"));
        assert_eq!(image_type(b"\x89PNG\r\n"), Some("image/png"));
        assert_eq!(image_type(b"GIF89a"), Some("image/gif"));
        assert_eq!(image_type(b"RIFF\0\0\0\0WEBPVP8 "), Some("image/webp"));
        assert_eq!(image_type(b"<html>"), None);
    }

    #[test]
    fn settings_come_from_the_environment() {
        let env = |pairs: &'static [(&'static str, &'static str)]| {
            move |name: &str| {
                pairs
                    .iter()
                    .find(|(k, _)| *k == name)
                    .map(|(_, v)| (*v).to_owned())
            }
        };
        let off = Settings::from_env(env(&[])).unwrap();
        assert!(!off.enabled && !off.lookup);
        assert_eq!(off.cache_dir, PathBuf::from("cache/covers"));
        let on = Settings::from_env(env(&[
            ("COVER_ART", "on"),
            ("COVER_ART_LOOKUP", "on"),
            ("COVER_ART_CACHE", "/var/cache/musicbanana/covers"),
        ]))
        .unwrap();
        assert!(on.enabled && on.lookup);
        assert_eq!(on.cache_dir, PathBuf::from("/var/cache/musicbanana/covers"));
        assert!(Settings::from_env(env(&[("COVER_ART", "yes")])).is_err());
        assert!(Settings::from_env(env(&[("COVER_ART_LOOKUP", "on")])).is_err());
        assert_eq!(off.cache_max, DEFAULT_CACHE_MAX);
        let small = Settings::from_env(env(&[("COVER_ART_CACHE_MAX", "200M")])).unwrap();
        assert_eq!(small.cache_max, 200 << 20);
        assert!(Settings::from_env(env(&[("COVER_ART_CACHE_MAX", "5M")])).is_err());
        assert!(Settings::from_env(env(&[("COVER_ART_CACHE_MAX", "lots")])).is_err());
        let blank = Settings::from_env(env(&[("COVER_ART_CACHE_MAX", " ")])).unwrap();
        assert_eq!(blank.cache_max, DEFAULT_CACHE_MAX);
    }

    #[test]
    fn sizes_are_read_in_steps_of_1024() {
        assert_eq!(parse_size("1234").unwrap(), 1234);
        assert_eq!(parse_size("1234B").unwrap(), 1234);
        assert_eq!(parse_size("64K").unwrap(), 64 << 10);
        assert_eq!(parse_size("500M").unwrap(), 500 << 20);
        assert_eq!(parse_size("500 MB").unwrap(), 500 << 20);
        assert_eq!(parse_size(" 2GiB ").unwrap(), 2 << 30);
        assert_eq!(parse_size("2g").unwrap(), 2 << 30);
        assert_eq!(parse_size("1T").unwrap(), 1 << 40);
        for wrong in [
            "",
            "M",
            "1.5G",
            "-1G",
            "1 PB",
            "10 apples",
            "99999999999999999999",
        ] {
            assert!(parse_size(wrong).is_err(), "{wrong}");
        }
        assert!(parse_size("99999999T").is_err());
    }

    fn url(text: &str) -> Url {
        Url::parse(text).unwrap()
    }

    fn allowed(target: &str) -> bool {
        redirect_allowed(
            &url(target),
            &url("https://coverartarchive.org/"),
            &["archive.org".to_owned()],
        )
    }

    #[test]
    fn redirects_go_to_the_archive_and_archive_org_only() {
        for good in [
            "https://coverartarchive.org/release-group/x/front-250",
            "https://archive.org/download/mbid-x/x.jpg",
            "https://ia800100.us.archive.org/12/items/mbid-x/x.jpg",
            "https://ARCHIVE.ORG/x",
        ] {
            assert!(allowed(good), "{good}");
        }
        for bad in [
            // Plain HTTP, other ports, other hosts that only look alike.
            "http://archive.org/x",
            "https://archive.org:8443/x",
            "http://coverartarchive.org/x",
            "https://coverartarchive.org:444/x",
            "https://evilarchive.org/x",
            "https://archive.org.evil.example/x",
            "https://archive.org@evil.example/x",
            "https://user:pw@archive.org/x",
            "https://.archive.org/x",
            "https://localhost/x",
            "https://127.0.0.1/x",
            "https://169.254.169.254/latest/meta-data",
            "https://[::1]/x",
            "file:///etc/passwd",
            "ftp://archive.org/x",
        ] {
            assert!(!allowed(bad), "{bad}");
        }
        // No allowed hosts: only the archive itself; empty entries allow nothing.
        let archive = url("https://coverartarchive.org/");
        assert!(!redirect_allowed(
            &url("https://archive.org/x"),
            &archive,
            &[]
        ));
        assert!(!redirect_allowed(
            &url("https://archive.org/x"),
            &archive,
            &[String::new()]
        ));
        assert!(redirect_allowed(
            &url("https://archive.org/x"),
            &archive,
            &[".archive.org.".into()]
        ));
    }

    #[test]
    fn logged_addresses_hold_no_secrets_or_line_breaks() {
        assert_eq!(
            loggable(&url(
                "https://user:secret@evil.example:8080/a/b?token=abc#frag"
            )),
            "https://evil.example:8080/a/b"
        );
        let long = format!("https://evil.example/{}", "a".repeat(500));
        let shown = loggable(&url(&long));
        assert_eq!(shown.chars().count(), LOGGED_URL_CHARS + 1);
        assert!(shown.ends_with('…'));
        // Percent-encoded by the parser already; nothing raw gets through.
        let shown = loggable(&url("https://evil.example/a%0Ab/\u{7f}ä"));
        assert!(shown.chars().all(|c| c.is_ascii_graphic()), "{shown}");
    }

    #[test]
    fn exact_matches_only() {
        let found: SearchResult = serde_json::from_value(serde_json::json!({
            "release-groups": [
                {"id": "00000000-0000-4000-8000-000000000001", "score": 100, "title": "Post (Live)",
                 "artist-credit": [{"name": "Björk", "artist": {"name": "Björk"}}]},
                {"id": "00000000-0000-4000-8000-000000000002", "score": 80, "title": "Post",
                 "artist-credit": [{"name": "Björk", "artist": {"name": "Björk"}}]},
                {"id": "00000000-0000-4000-8000-000000000003", "score": 95, "title": "Post",
                 "artist-credit": [{"name": "Bjork", "joinphrase": " & ", "artist": {"name": "Björk"}},
                                   {"name": "Friends", "artist": {"name": "Friends"}}]}
            ]
        }))
        .unwrap();
        assert_eq!(
            best_match(found, "bjork & friends", "POST"),
            Some(Uuid::parse_str("00000000-0000-4000-8000-000000000003").unwrap())
        );
        let none: SearchResult = serde_json::from_str("{}").unwrap();
        assert_eq!(best_match(none, "Björk", "Post"), None);
    }

    mod properties {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            // Whatever the archive sends us to, only https to archive.org (or a
            // subdomain) on the default port, or the archive itself, passes.
            #[test]
            fn only_allowed_hosts_pass(
                scheme in prop::sample::select(vec!["http", "https", "ftp", "file"]),
                host in "[a-z0-9.-]{1,30}",
                port in prop::option::of(1u16..),
                user in prop::option::of("[a-z]{1,5}"),
                path in "[a-zA-Z0-9/._%-]{0,40}",
            ) {
                let auth = user.map(|u| format!("{u}@")).unwrap_or_default();
                let port_text = port.map(|p| format!(":{p}")).unwrap_or_default();
                let Ok(target) = Url::parse(&format!("{scheme}://{auth}{host}{port_text}/{path}")) else {
                    return Ok(());
                };
                if allowed(target.as_str()) {
                    let h = target.host_str().unwrap();
                    prop_assert_eq!(target.scheme(), "https");
                    prop_assert!(target.username().is_empty());
                    prop_assert!(target.port().is_none());
                    prop_assert!(
                        h == "coverartarchive.org" || h == "archive.org" || h.ends_with(".archive.org"),
                        "{}", h
                    );
                }
            }

            #[test]
            fn logged_addresses_are_one_short_printable_line(text in "\\PC{0,300}") {
                for candidate in [format!("https://example.org/{text}"), format!("https://{text}/")] {
                    if let Ok(target) = Url::parse(&candidate) {
                        let shown = loggable(&target);
                        prop_assert!(shown.chars().count() <= LOGGED_URL_CHARS + 1);
                        prop_assert!(shown.chars().all(|c| c.is_ascii_graphic() || c == '…'));
                        prop_assert!(!shown.contains('?') && !shown.contains('#'));
                    }
                }
            }

            #[test]
            fn sizes_never_panic(text in "\\PC{0,20}") {
                let _ = parse_size(&text);
            }

            #[test]
            fn sizes_read_back(n in 0u64..1_000_000, unit in prop::sample::select(vec![("", 0), ("K", 10), ("M", 20), ("G", 30)])) {
                prop_assert_eq!(parse_size(&format!("{n}{}", unit.0)).unwrap(), n << unit.1);
            }

            // Only the four picture formats, judged by their first bytes.
            #[test]
            fn only_pictures_are_recognized(bytes in prop::collection::vec(any::<u8>(), 0..64)) {
                if let Some(kind) = image_type(&bytes) {
                    let ok = match kind {
                        "image/jpeg" => bytes.starts_with(&[0xff, 0xd8, 0xff]),
                        "image/png" => bytes.starts_with(b"\x89PNG"),
                        "image/gif" => bytes.starts_with(b"GIF8"),
                        "image/webp" => bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP"),
                        _ => false,
                    };
                    prop_assert!(ok, "{} for {:?}", kind, bytes);
                }
            }

            #[test]
            fn markup_is_never_a_picture(text in "\\s*<[a-zA-Z!?].{0,40}") {
                prop_assert_eq!(image_type(text.as_bytes()), None);
            }

            // Inside the quotes of a search, nothing can end the phrase early.
            #[test]
            fn phrases_stay_phrases(text in "\\PC{0,40}") {
                let escaped = lucene_phrase(&text);
                let mut chars = escaped.chars();
                let mut back = String::new();
                while let Some(c) = chars.next() {
                    prop_assert_ne!(c, '"', "unescaped quote in {}", escaped);
                    if c == '\\' {
                        let next = chars.next();
                        prop_assert!(matches!(next, Some('\\' | '"')), "{}", escaped);
                        back.push(next.unwrap());
                    } else {
                        back.push(c);
                    }
                }
                prop_assert_eq!(back, text);
            }

            #[test]
            fn names_normalize_once(text in "\\PC{0,40}") {
                let once = normalize(&text);
                prop_assert_eq!(normalize(&once), once.clone());
                prop_assert!(!once.starts_with(' ') && !once.ends_with(' ') && !once.contains("  "));
            }

            // Cache file names are 32 hex digits, so no name can leave the directory.
            #[test]
            fn search_keys_are_plain_file_names(artist in "\\PC{0,40}", title in "\\PC{0,40}") {
                let key = search_key(&artist, &title);
                prop_assert_eq!(key.len(), 32);
                prop_assert!(key.chars().all(|c| c.is_ascii_hexdigit()));
                if artist.is_ascii() {
                    prop_assert_eq!(key, search_key(&artist.to_uppercase(), &title));
                }
            }

            // Whatever MusicBrainz answers, a match has the title asked for.
            #[test]
            fn matches_have_the_title(
                titles in prop::collection::vec("[a-c ]{0,4}", 0..6),
                scores in prop::collection::vec(0u32..=100, 6),
                title in "[a-c ]{0,4}",
            ) {
                let groups: Vec<_> = titles.iter().enumerate().map(|(i, t)| serde_json::json!({
                    "id": Uuid::from_u128(i as u128 + 1), "score": scores[i], "title": t,
                    "artist-credit": [{"name": "A", "artist": {"name": "A"}}]
                })).collect();
                let found: SearchResult =
                    serde_json::from_value(serde_json::json!({"release-groups": groups})).unwrap();
                if let Some(id) = best_match(found, "a", &title) {
                    let i = id.as_u128() as usize - 1;
                    prop_assert_eq!(normalize(&titles[i]), normalize(&title));
                    prop_assert!(scores[i] >= 90);
                }
            }
        }
    }
}
