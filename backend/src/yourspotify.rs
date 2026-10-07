//! Spotify listens from YourSpotify (github.com/Yooooomi/your_spotify), which
//! keeps the history of a Spotify account: the plays from the account's data
//! export and those it has fetched from Spotify since.
//!
//! YourSpotify has no documented API. This uses the route its web interface shows
//! the history with, `GET /spotify/gethistory`, which returns the plays of a time
//! range newest first, at most 20 at a time, each with its track, album and
//! artists. The public token from YourSpotify's settings stands in for a login;
//! it gives read access to all statistics of the account, so it is never logged
//! or shown.

use std::{collections::HashMap, fmt};

use anyhow::{Context, bail};
use reqwest::{StatusCode, Url};
use serde::Deserialize;
use serde_json::json;
use sqlx::PgPool;
use time::{Duration, OffsetDateTime, UtcOffset, macros::datetime};

use crate::{
    catalog::Mbids,
    scrobble::{self, Listen},
};

/// What the imported listens give as their client.
pub const CLIENT: &str = "Spotify via YourSpotify";

/// The most plays YourSpotify returns per request.
const PAGE: usize = 20;

/// Listens stored per transaction.
const BATCH: usize = 1000;

/// The history is fetched in time windows, oldest first, and each window is
/// stored before the next one is fetched. A window is a month, or twice as long
/// as the one before when that had no plays, up to a year, so that the years
/// before the first play take few requests.
const WINDOW: Duration = Duration::days(30);
const LONGEST_WINDOW: Duration = Duration::days(365);

/// Spotify started in 2008; anything older comes in one window.
const SPOTIFY_STARTED: OffsetDateTime = datetime!(2008-01-01 0:00 UTC);

/// The YourSpotify addresses accounts may connect in their settings (from
/// YOURSPOTIFY_ALLOWED_URLS). An address allows itself and the paths below it,
/// with the same scheme, host and port. The command line takes any address.
#[derive(Debug, Default)]
pub struct Allowlist(Vec<Url>);

impl Allowlist {
    /// Addresses separated by commas or white space.
    pub fn parse(list: &str) -> anyhow::Result<Self> {
        list.split(|c: char| c == ',' || c.is_whitespace())
            .filter(|a| !a.is_empty())
            .map(|a| {
                let url = base_url(a)?;
                if url.query().is_some() || !url.username().is_empty() || url.password().is_some() {
                    bail!("{a} should be a plain address, such as http://yourspotify:8080");
                }
                Ok(url)
            })
            .collect::<anyhow::Result<_>>()
            .map(Self)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn addresses(&self) -> Vec<String> {
        self.0
            .iter()
            .map(|u| u.as_str().trim_end_matches('/').to_owned())
            .collect()
    }

    pub fn allows(&self, api: &str) -> bool {
        let Ok(url) = base_url(api) else {
            return false;
        };
        url.username().is_empty()
            && url.password().is_none()
            && url.query().is_none()
            && self.0.iter().any(|allowed| {
                url.scheme() == allowed.scheme()
                    && url.host() == allowed.host()
                    && url.port_or_known_default() == allowed.port_or_known_default()
                    && url.path().starts_with(allowed.path())
            })
    }
}

/// `api` with a slash at the end, so that paths below it join on.
fn base_url(api: &str) -> anyhow::Result<Url> {
    let api = api.trim();
    let base = Url::parse(&format!("{}/", api.trim_end_matches('/')))
        .with_context(|| format!("{api} is not an address"))?;
    if !matches!(base.scheme(), "http" | "https") {
        bail!("{api} is not an http(s) address");
    }
    Ok(base)
}

/// A YourSpotify server and the token to read an account's history with.
pub struct Source {
    http: reqwest::Client,
    history: Url,
    /// The address as given, for messages.
    shown: String,
    token: String,
}

impl Source {
    /// `api` is the address of YourSpotify's API (its API_ENDPOINT), such as
    /// `http://yourspotify:8080` or `https://spotify.example.org/api`.
    pub fn new(api: &str, token: &str) -> anyhow::Result<Self> {
        let shown = api.trim().trim_end_matches('/').to_owned();
        let base = base_url(api)?;
        let token = token.trim();
        if token.is_empty() {
            bail!("the YourSpotify token is empty");
        }
        // musicbanana uses rustls with ring (as sqlx does) and the system's
        // certificates; reqwest wants the crypto provider set up front.
        let _ = rustls::crypto::ring::default_provider().install_default();
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            // Only the address that was allowed, not where it might send us on.
            .redirect(reqwest::redirect::Policy::none())
            .user_agent(concat!("musicbanana/", env!("CARGO_PKG_VERSION")))
            .build()?;
        Ok(Self {
            http,
            history: base.join("spotify/gethistory")?,
            shown,
            token: token.to_owned(),
        })
    }

    /// Asks YourSpotify for its latest plays, to see that address and token work.
    pub async fn check(&self) -> anyhow::Result<()> {
        self.page(0, None).await.map(|_| ())
    }

    /// The plays from `offset` on, newest first, of those from `from` up to
    /// (without) `to`, or of all.
    async fn page(
        &self,
        offset: usize,
        range: Option<(OffsetDateTime, OffsetDateTime)>,
    ) -> anyhow::Result<Vec<Play>> {
        let mut request = self
            .http
            .get(self.history.clone())
            .query(&[("token", self.token.as_str())])
            .query(&[("number", PAGE), ("offset", offset)]);
        if let Some((from, to)) = range {
            // YourSpotify takes the plays after `start` and before `end`, to the
            // millisecond as JavaScript dates have it.
            request = request.query(&[
                ("start", js_date(from - Duration::milliseconds(1))),
                ("end", js_date(to)),
            ]);
        }
        let response = request
            .send()
            .await
            // The error would show the address, token included.
            .map_err(reqwest::Error::without_url)
            .with_context(|| format!("asking YourSpotify at {} for its history", self.shown))?;
        match response.status() {
            StatusCode::OK => {}
            StatusCode::UNAUTHORIZED => bail!(
                "YourSpotify at {} does not take the token; the public token is in \
                 YourSpotify's settings",
                self.shown
            ),
            status => bail!(
                "YourSpotify at {} answered {status} when asked for its history",
                self.shown
            ),
        }
        let body = response
            .bytes()
            .await
            .map_err(reqwest::Error::without_url)
            .with_context(|| format!("reading the history from YourSpotify at {}", self.shown))?;
        serde_json::from_slice(&body).with_context(|| {
            format!(
                "{} did not answer with YourSpotify's history; it has to be the address \
                 of YourSpotify's API (API_ENDPOINT), not of its web interface",
                self.shown
            )
        })
    }

    /// The plays from `from` up to (without) `to`, oldest first, or `None` when
    /// YourSpotify does not take a time range.
    async fn window(
        &self,
        from: OffsetDateTime,
        to: OffsetDateTime,
    ) -> anyhow::Result<Option<Vec<Play>>> {
        let mut plays = Vec::new();
        for offset in (0..).step_by(PAGE) {
            let page = self.page(offset, Some((from, to))).await?;
            let full = page.len() == PAGE;
            if page
                .iter()
                .any(|play| play.played_at < from || play.played_at >= to)
            {
                return Ok(None);
            }
            plays.extend(page);
            if !full {
                break;
            }
        }
        Ok(Some(oldest_first(plays)))
    }

    /// The plays from `from` on, or all, oldest first, fetched newest first
    /// without a time range.
    async fn newest_first(&self, from: Option<OffsetDateTime>) -> anyhow::Result<Vec<Play>> {
        let mut plays = Vec::new();
        for offset in (0..).step_by(PAGE) {
            let page = self.page(offset, None).await?;
            let full = page.len() == PAGE;
            let mut reached_since = false;
            for play in page {
                if from.is_some_and(|from| play.played_at < from) {
                    reached_since = true;
                } else {
                    plays.push(play);
                }
            }
            if !full || reached_since {
                break;
            }
            if (offset + PAGE).is_multiple_of(2000) {
                tracing::info!("fetched {} plays from YourSpotify so far", offset + PAGE);
            }
        }
        Ok(oldest_first(plays))
    }
}

/// Plays YourSpotify fetched from Spotify meanwhile move the others down a page,
/// so some come twice.
fn oldest_first(mut plays: Vec<Play>) -> Vec<Play> {
    plays.sort_by_key(|play| play.played_at);
    plays.dedup_by_key(|play| play.played_at);
    plays
}

/// "2026-10-03T14:30:00.250Z", as JavaScript's Date writes and reads it.
fn js_date(at: OffsetDateTime) -> String {
    let at = at.to_offset(UtcOffset::UTC);
    format!(
        "{}T{:02}:{:02}:{:02}.{:03}Z",
        at.date(),
        at.hour(),
        at.minute(),
        at.second(),
        at.millisecond()
    )
}

/// One play as YourSpotify returns it, with the parts used here.
#[derive(Deserialize)]
struct Play {
    #[serde(with = "time::serde::rfc3339")]
    played_at: OffsetDateTime,
    track: Track,
}

#[derive(Deserialize)]
struct Track {
    id: String,
    name: String,
    /// Spotify IDs of the artists, the main one first.
    #[serde(default)]
    artists: Vec<String>,
    #[serde(default)]
    duration_ms: Option<i64>,
    #[serde(default)]
    track_number: Option<i64>,
    #[serde(default)]
    full_album: Option<Album>,
    /// The artists, in no particular order.
    #[serde(default)]
    full_artists: Vec<Artist>,
}

#[derive(Deserialize)]
struct Album {
    id: String,
    name: String,
}

#[derive(Deserialize)]
struct Artist {
    id: String,
    name: String,
}

impl Play {
    /// The listen, or `None` when YourSpotify knows none of its artists.
    fn listen(self) -> Option<Listen> {
        let Track {
            id,
            name,
            artists,
            duration_ms,
            track_number,
            full_album,
            full_artists,
        } = self.track;
        let mut names: HashMap<_, _> = full_artists.into_iter().map(|a| (a.id, a.name)).collect();
        let artists: Vec<(String, String)> = artists
            .into_iter()
            .filter_map(|id| names.remove(&id).map(|name| (id, name)))
            .collect();
        let artist = artists.first()?.1.clone();
        let spotify = |kind: &str, id: &str| format!("https://open.spotify.com/{kind}/{id}");
        // Named as in ListenBrainz's additional_info.
        let mut extra = json!({
            "music_service": "spotify.com",
            "spotify_id": spotify("track", &id),
            "spotify_artist_ids": artists.iter().map(|(id, _)| spotify("artist", id)).collect::<Vec<_>>(),
            "artist_names": artists.iter().map(|(_, name)| name).collect::<Vec<_>>(),
        });
        if let Some(album) = &full_album {
            extra["spotify_album_id"] = spotify("album", &album.id).into();
        }
        Some(Listen {
            listened_at: self.played_at,
            artist,
            track: name,
            album: full_album.map(|album| album.name),
            track_number: track_number.and_then(|n| n.try_into().ok()),
            duration_ms: duration_ms.and_then(|n| n.try_into().ok()),
            client: Some(CLIENT.to_owned()),
            mbids: Mbids::default(),
            extra: Some(extra),
        })
    }
}

/// The profile `slug` of the account `username`.
pub async fn profile_id(db: &PgPool, username: &str, slug: &str) -> anyhow::Result<i64> {
    sqlx::query_scalar!(
        "SELECT p.id
           FROM profile p
           JOIN account a ON a.id = p.account_id
          WHERE a.username = $1::text::citext AND p.slug = $2::text::citext",
        username,
        slug,
    )
    .fetch_optional(db)
    .await?
    .with_context(|| format!("user {username} has no profile {slug}"))
}

/// What an import brought.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    /// The latest listen imported before, where this import started.
    pub since: Option<OffsetDateTime>,
    /// Plays newer than that.
    pub plays: usize,
    /// Those stored as listens; the others were in the profile already.
    pub recorded: u64,
    /// Plays left out because YourSpotify knows none of their artists.
    pub without_artist: usize,
    pub first: Option<OffsetDateTime>,
    pub last: Option<OffsetDateTime>,
}

/// Where an import starts.
#[derive(Clone, Copy, Debug)]
pub enum Start {
    /// After the latest play imported before, or at the beginning.
    Latest,
    /// At the beginning of the history.
    Beginning,
    /// At this time.
    At(OffsetDateTime),
}

/// Imports the plays from YourSpotify into a profile: the whole history the first
/// time, then those newer than the latest one imported, or the whole history
/// again with `all`. Plays the profile has already are skipped either way, as
/// are those that look like a second scrobble of a listen (see [`scrobble::record`]).
pub async fn import(
    db: &PgPool,
    source: &Source,
    profile_id: i64,
    all: bool,
) -> anyhow::Result<Report> {
    let start = if all { Start::Beginning } else { Start::Latest };
    import_from(db, source, profile_id, start, |_, _| async { Ok(()) }).await
}

/// Imports the plays from `start` on into a profile, skipping those it has
/// already (see [`import`]).
///
/// The plays are stored oldest first, a window of time at once, so they show up
/// in the profile while the import runs, and one that stops halfway leaves no gap
/// behind the latest listen: the next one goes on from there. After each window
/// with plays, and at the end, `progress` hears the time up to which the plays
/// are done, and the report so far.
pub async fn import_from<F>(
    db: &PgPool,
    source: &Source,
    profile_id: i64,
    start: Start,
    mut progress: impl FnMut(OffsetDateTime, Report) -> F,
) -> anyhow::Result<Report>
where
    F: Future<Output = anyhow::Result<()>>,
{
    let since = match start {
        Start::Latest => {
            sqlx::query_scalar!(
                "SELECT max(listened_at) FROM listen WHERE profile_id = $1 AND client = $2",
                profile_id,
                CLIENT,
            )
            .fetch_one(db)
            .await?
        }
        Start::Beginning | Start::At(_) => None,
    };
    let start = match start {
        Start::At(at) => Some(at),
        _ => since.map(|since| since + Duration::milliseconds(1)),
    };
    let mut report = Report {
        since,
        ..Report::default()
    };
    // Whatever YourSpotify has, even with a clock ahead of ours.
    let until = OffsetDateTime::now_utc() + Duration::days(1);
    let mut from = start.unwrap_or(OffsetDateTime::UNIX_EPOCH);
    let mut length = WINDOW;
    let mut logged = 0;
    while from < until {
        let to = if from < SPOTIFY_STARTED {
            SPOTIFY_STARTED
        } else {
            (from + length).min(until)
        };
        let Some(plays) = source.window(from, to).await? else {
            tracing::info!(
                "this YourSpotify takes no time range, so its history comes all at once, \
                 newest first"
            );
            let plays = source.newest_first(start).await?;
            store(db, profile_id, plays, &mut report).await?;
            progress(until, report.clone()).await?;
            return Ok(report);
        };
        length = if plays.is_empty() {
            (length * 2_i32).min(LONGEST_WINDOW)
        } else {
            WINDOW
        };
        let found = !plays.is_empty();
        store(db, profile_id, plays, &mut report).await?;
        if found {
            progress(to, report.clone()).await?;
        }
        if let Some(last) = report.last.filter(|_| report.plays / 2000 > logged) {
            logged = report.plays / 2000;
            tracing::info!(
                "imported {} Spotify plays from YourSpotify so far, up to {}",
                report.recorded,
                last.date()
            );
        }
        from = to;
    }
    progress(until, report.clone()).await?;
    Ok(report)
}

/// Stores plays given oldest first.
async fn store(
    db: &PgPool,
    profile_id: i64,
    plays: Vec<Play>,
    report: &mut Report,
) -> anyhow::Result<()> {
    let (Some(first), Some(last)) = (plays.first(), plays.last()) else {
        return Ok(());
    };
    report.first = report.first.or(Some(first.played_at));
    report.last = Some(last.played_at);
    let count = plays.len();
    report.plays += count;
    let listens: Vec<Listen> = plays.into_iter().filter_map(Play::listen).collect();
    report.without_artist += count - listens.len();
    for batch in listens.chunks(BATCH) {
        report.recorded += scrobble::record(db, profile_id, batch).await?;
    }
    Ok(())
}

impl fmt::Display for Report {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (Some(first), Some(last)) = (self.first, self.last) else {
            return match self.since {
                Some(since) => write!(
                    f,
                    "No Spotify plays in YourSpotify after {}.",
                    minutes(since)
                ),
                None => write!(f, "YourSpotify has no Spotify plays."),
            };
        };
        write!(
            f,
            "Found {} Spotify plays in YourSpotify from {} to {}: imported {}",
            self.plays,
            minutes(first),
            minutes(last),
            self.recorded,
        )?;
        let known = self.plays as u64 - self.without_artist as u64 - self.recorded;
        if known > 0 {
            write!(f, ", {known} were in the profile already")?;
        }
        if self.without_artist > 0 {
            write!(
                f,
                ", {} left out as YourSpotify knows none of their artists",
                self.without_artist
            )?;
        }
        write!(f, ".")
    }
}

/// "2026-10-03 14:30 UTC"
fn minutes(at: OffsetDateTime) -> String {
    let at = at.to_offset(time::UtcOffset::UTC);
    format!("{} {:02}:{:02} UTC", at.date(), at.hour(), at.minute())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allowlist() {
        let allowed =
            Allowlist::parse("http://yourspotify:8080, https://music.example.org/spotify/api/")
                .unwrap();
        for fine in [
            "http://yourspotify:8080",
            "http://YourSpotify:8080/",
            "http://yourspotify:8080/api",
            "https://music.example.org/spotify/api",
            "https://music.example.org:443/spotify/api/",
        ] {
            assert!(allowed.allows(fine), "{fine}");
        }
        for not in [
            "http://yourspotify",
            "https://yourspotify:8080",
            "http://yourspotify:8081",
            "http://yourspotify.evil:8080",
            "http://yourspotify:8080@evil:8080",
            "http://evil/?http://yourspotify:8080",
            "https://music.example.org/spotify",
            "https://music.example.org/spotify/apix",
            "file:///etc/passwd",
            "",
        ] {
            assert!(!allowed.allows(not), "{not}");
        }
        assert!(!Allowlist::default().allows("http://yourspotify:8080"));
        assert!(Allowlist::parse("").unwrap().is_empty());
        assert!(Allowlist::parse("yourspotify:8080").is_err());
        assert!(Allowlist::parse("http://user:pw@yourspotify:8080").is_err());
    }
}
