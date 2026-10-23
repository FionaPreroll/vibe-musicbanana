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
const MAX_BYTES: usize = 5 * 1024 * 1024;

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
    /// The Cover Art Archive; tests point it at a stand-in.
    pub archive: Url,
    /// MusicBrainz's web service.
    pub musicbrainz: Url,
    /// The time between two searches at MusicBrainz, which allows one a second.
    pub search_interval: Duration,
}

impl Settings {
    /// Off, with the real archive and MusicBrainz once switched on.
    pub fn off(cache_dir: impl Into<PathBuf>) -> Self {
        Self {
            enabled: false,
            lookup: false,
            cache_dir: cache_dir.into(),
            archive: Url::parse("https://coverartarchive.org/").unwrap(),
            musicbrainz: Url::parse("https://musicbrainz.org/").unwrap(),
            search_interval: Duration::from_millis(1100),
        }
    }

    /// From `COVER_ART`, `COVER_ART_LOOKUP` and `COVER_ART_CACHE`.
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
        Ok(settings)
    }
}

/// Fetches and keeps covers; shared by all requests.
pub struct Covers {
    settings: Settings,
    http: OnceLock<reqwest::Client>,
    /// At most this many requests to the archive at once.
    fetching: Semaphore,
    /// Albums waiting for a search at MusicBrainz, by cache key.
    queued: Arc<Mutex<HashSet<String>>>,
    searches: OnceLock<mpsc::UnboundedSender<Search>>,
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

impl Default for Covers {
    fn default() -> Self {
        Self::new(Settings::off("cache/covers"))
    }
}

impl Covers {
    pub fn new(settings: Settings) -> Self {
        Self {
            settings,
            http: OnceLock::new(),
            fetching: Semaphore::new(4),
            queued: Arc::default(),
            searches: OnceLock::new(),
        }
    }

    pub fn enabled(&self) -> bool {
        self.settings.enabled
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    fn http(&self) -> &reqwest::Client {
        self.http.get_or_init(|| {
            // As in yourspotify.rs: reqwest wants the crypto provider set up front.
            let _ = rustls::crypto::ring::default_provider().install_default();
            reqwest::Client::builder()
                .timeout(Duration::from_secs(15))
                .user_agent(USER_AGENT)
                .build()
                .expect("an HTTP client")
        })
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
                if let Err(err) = write(&file, &image.bytes).await {
                    tracing::warn!("keeping the cover of {mbid}: {err:#}");
                }
                let _ = tokio::fs::remove_file(&missing).await;
                Some(image)
            }
            Ok(None) => {
                if let Err(err) = write(&missing, b"").await {
                    tracing::warn!("noting the missing cover of {mbid}: {err:#}");
                }
                None
            }
            Err(err) => {
                tracing::warn!("fetching the cover of {mbid}: {err:#}");
                None
            }
        }
    }

    async fn download(&self, mbid: Uuid, size: u16) -> anyhow::Result<Option<Image>> {
        let url = self
            .settings
            .archive
            .join(&format!("release-group/{mbid}/front-{size}"))?;
        let response = self
            .http()
            .get(url)
            .send()
            .await
            .context("asking the Cover Art Archive")?;
        match response.status() {
            StatusCode::OK => {}
            // No release of the group has a front cover, or MusicBrainz does not
            // know the ID (any more).
            StatusCode::NOT_FOUND | StatusCode::BAD_REQUEST => return Ok(None),
            status => bail!("the Cover Art Archive answered {status}"),
        }
        if response
            .content_length()
            .is_some_and(|n| n > MAX_BYTES as u64)
        {
            bail!("the cover is larger than {MAX_BYTES} bytes");
        }
        let bytes = response.bytes().await.context("reading the cover")?;
        if bytes.len() > MAX_BYTES {
            bail!("the cover is larger than {MAX_BYTES} bytes");
        }
        let content_type = image_type(&bytes).context("the archive sent no image")?;
        Ok(Some(Image {
            bytes,
            content_type,
        }))
    }

    /// The release group a search by artist and title found, if one was made.
    /// Otherwise the search is queued, and `Err(())` says to ask again later.
    pub async fn looked_up(&self, artist: &str, title: &str) -> Result<Option<Uuid>, ()> {
        let key = search_key(artist, title);
        let file = self.search_file(&key);
        if let Ok(text) = tokio::fs::read(&file).await
            && let Ok(found) = serde_json::from_slice::<Found>(&text)
            && (found.mbid.is_some() || fresh(&file).await)
        {
            return Ok(found.mbid);
        }
        self.queue(Search {
            key,
            artist: artist.to_owned(),
            title: title.to_owned(),
        });
        Err(())
    }

    fn search_file(&self, key: &str) -> PathBuf {
        self.settings
            .cache_dir
            .join("musicbrainz")
            .join(format!("{key}.json"))
    }

    fn queue(&self, search: Search) {
        if !self.queued.lock().unwrap().insert(search.key.clone()) {
            return;
        }
        let sender = self.searches.get_or_init(|| {
            let (sender, receiver) = mpsc::unbounded_channel();
            tokio::spawn(search_worker(
                receiver,
                self.settings.clone(),
                self.http().clone(),
                self.queued.clone(),
            ));
            sender
        });
        let _ = sender.send(search);
    }
}

/// Searches MusicBrainz one album at a time, keeping its interval.
async fn search_worker(
    mut searches: mpsc::UnboundedReceiver<Search>,
    settings: Settings,
    http: reqwest::Client,
    queued: Arc<Mutex<HashSet<String>>>,
) {
    let mut last: Option<Instant> = None;
    while let Some(search) = searches.recv().await {
        if let Some(last) = last {
            tokio::time::sleep_until((last + settings.search_interval).into()).await;
        }
        last = Some(Instant::now());
        let file = settings
            .cache_dir
            .join("musicbrainz")
            .join(format!("{}.json", search.key));
        match find_release_group(&http, &settings.musicbrainz, &search.artist, &search.title).await
        {
            Ok(mbid) => {
                let text = serde_json::to_vec(&Found { mbid }).unwrap();
                if let Err(err) = write(&file, &text).await {
                    tracing::warn!("keeping a MusicBrainz search: {err:#}");
                }
            }
            Err(err) => tracing::warn!(
                "searching MusicBrainz for {} – {}: {err:#}",
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
        .context("asking MusicBrainz")?;
    if response.status() != StatusCode::OK {
        bail!("MusicBrainz answered {}", response.status());
    }
    let found: SearchResult = response
        .json()
        .await
        .context("reading MusicBrainz's answer")?;
    let (artist, title) = (normalize(artist), normalize(title));
    Ok(found
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
        .map(|group| group.id))
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
            Ok(found) => mbids.extend(found),
            Err(()) => later = true,
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
    }
}
