//! Read-only API behind the public profile pages: overview, charts for any period,
//! the top artists of each year, recent listens and what is playing right now.
//!
//! A public profile is there for everyone, one for followers to its owner and the
//! accounts following it, a private one to its owner only. To anyone else such a
//! profile answers 404, like an unknown one.

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::get,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use time::{Date, Month, OffsetDateTime};

use crate::{AppError, AppState, account::Viewer};

// Days in query parameters, e.g. ?from=2009-06-01.
time::serde::format_description!(day, Date, "[year]-[month]-[day]");

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/profiles", get(list))
        .route("/profiles/{username}/{slug}", get(overview))
        .route("/profiles/{username}/{slug}/top/{kind}", get(top))
        .route(
            "/profiles/{username}/{slug}/top/artists/years",
            get(top_artists_per_year),
        )
        .route("/profiles/{username}/{slug}/listens", get(listens))
        .route("/profiles/{username}/{slug}/now-playing", get(now_playing))
}

#[derive(Serialize)]
struct ProfileSummary {
    username: String,
    slug: String,
    name: String,
    visibility: String,
    listens: i64,
}

/// The profiles the viewer may see.
async fn list(
    State(state): State<AppState>,
    Viewer(viewer): Viewer,
) -> Result<Json<Vec<ProfileSummary>>, AppError> {
    let profiles = sqlx::query_as!(
        ProfileSummary,
        r#"SELECT a.username::text AS "username!", p.slug::text AS "slug!", p.name,
                  p.visibility::text AS "visibility!",
                  (SELECT count(*) FROM listen l WHERE l.profile_id = p.id) AS "listens!"
             FROM profile p
             JOIN account a ON a.id = p.account_id
            WHERE p.visibility = 'public' OR p.account_id = $1
               OR (p.visibility = 'followers'
                   AND EXISTS (SELECT FROM follow f
                                WHERE f.profile_id = p.id AND f.follower_id = $1))
            ORDER BY a.username, p.slug"#,
        viewer,
    )
    .fetch_all(&state.db)
    .await?;
    Ok(Json(profiles))
}

pub(crate) struct Profile {
    pub id: i64,
    pub username: String,
    pub slug: String,
    pub name: String,
    pub visibility: String,
    /// Whether it belongs to the viewer.
    pub own: bool,
}

/// The profile, if the viewer may see it.
pub(crate) async fn find_profile(
    db: &PgPool,
    Viewer(viewer): Viewer,
    username: &str,
    slug: &str,
) -> Result<Profile, AppError> {
    // Cast the parameters to citext: comparing citext to text would compare case-sensitively.
    sqlx::query_as!(
        Profile,
        r#"SELECT p.id, a.username::text AS "username!", p.slug::text AS "slug!", p.name,
                  p.visibility::text AS "visibility!",
                  p.account_id IS NOT DISTINCT FROM $3 AS "own!"
             FROM profile p
             JOIN account a ON a.id = p.account_id
            WHERE a.username = $1::text::citext
              AND p.slug = $2::text::citext
              AND (p.visibility = 'public' OR p.account_id = $3
                   OR (p.visibility = 'followers'
                       AND EXISTS (SELECT FROM follow f
                                    WHERE f.profile_id = p.id AND f.follower_id = $3)))"#,
        username,
        slug,
        viewer,
    )
    .fetch_optional(db)
    .await?
    .ok_or(AppError::NotFound)
}

#[derive(Deserialize)]
struct OverviewParams {
    /// IANA time zone for the year boundaries, e.g. Europe/Berlin. Defaults to UTC.
    tz: Option<String>,
}

#[derive(Serialize)]
struct Overview {
    username: String,
    slug: String,
    name: String,
    visibility: String,
    /// Whether it belongs to the viewer.
    own: bool,
    listens: i64,
    #[serde(with = "time::serde::rfc3339::option")]
    first_listened_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339::option")]
    last_listened_at: Option<OffsetDateTime>,
    years: Vec<YearCount>,
}

#[derive(Serialize)]
struct YearCount {
    year: i32,
    listens: i64,
}

async fn overview(
    State(state): State<AppState>,
    viewer: Viewer,
    Path((username, slug)): Path<(String, String)>,
    Query(params): Query<OverviewParams>,
) -> Result<Json<Overview>, AppError> {
    let profile = find_profile(&state.db, viewer, &username, &slug).await?;
    let tz = time_zone(&state.db, params.tz).await?;

    let years = sqlx::query_as!(
        YearCount,
        r#"SELECT date_part('year', listened_at AT TIME ZONE $2)::int AS "year!",
                  count(*) AS "listens!"
             FROM listen
            WHERE profile_id = $1
            GROUP BY 1
            ORDER BY 1"#,
        profile.id,
        tz,
    )
    .fetch_all(&state.db)
    .await?;

    let span = sqlx::query!(
        "SELECT min(listened_at) AS first, max(listened_at) AS last FROM listen WHERE profile_id = $1",
        profile.id,
    )
    .fetch_one(&state.db)
    .await?;

    Ok(Json(Overview {
        username: profile.username,
        slug: profile.slug,
        name: profile.name,
        visibility: profile.visibility,
        own: profile.own,
        listens: years.iter().map(|y| y.listens).sum(),
        first_listened_at: span.first,
        last_listened_at: span.last,
        years,
    }))
}

#[derive(Deserialize)]
struct TopParams {
    /// Calendar year in `tz`; short for from=<year>-01-01&to=<year>-12-31.
    year: Option<i32>,
    /// First day of the period in `tz`; open when missing.
    #[serde(default, with = "day::option")]
    from: Option<Date>,
    /// Last day of the period in `tz`, inclusive; open when missing.
    #[serde(default, with = "day::option")]
    to: Option<Date>,
    tz: Option<String>,
    limit: Option<i64>,
}

impl TopParams {
    /// The first and last day of the period; `None` leaves that end open, so
    /// neither of them means all time.
    fn days(&self) -> Result<(Option<Date>, Option<Date>), AppError> {
        let bad = |message: &str| AppError::BadRequest(message.into());
        if let Some(year) = self.year {
            if self.from.is_some() || self.to.is_some() {
                return Err(bad("year cannot be combined with from or to"));
            }
            if !(1..=9999).contains(&year) {
                return Err(bad("year must be between 1 and 9999"));
            }
            let day =
                |month, day| Date::from_calendar_date(year, month, day).map_err(AppError::from);
            return Ok((
                Some(day(Month::January, 1)?),
                Some(day(Month::December, 31)?),
            ));
        }
        if [self.from, self.to].iter().flatten().any(|d| d.year() < 1) {
            return Err(bad("dates must not be before the year 1"));
        }
        if let (Some(from), Some(to)) = (self.from, self.to)
            && from > to
        {
            return Err(bad("from must not be after to"));
        }
        Ok((self.from, self.to))
    }
}

#[derive(Serialize)]
pub(crate) struct ChartEntry {
    pub id: i64,
    pub name: String,
    /// The artist of a release or recording; missing in the artist chart.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artist: Option<String>,
    pub listens: i64,
}

// All three charts count listens in [lo, hi): from midnight at the start of the
// first day to midnight after the last one in the given time zone, without a
// bound where the period is open.
async fn top(
    State(state): State<AppState>,
    viewer: Viewer,
    Path((username, slug, kind)): Path<(String, String, String)>,
    Query(params): Query<TopParams>,
) -> Result<Json<Vec<ChartEntry>>, AppError> {
    let (from, to) = params.days()?;
    let profile = find_profile(&state.db, viewer, &username, &slug).await?;
    let tz = time_zone(&state.db, params.tz).await?;
    let limit = params.limit.unwrap_or(10).clamp(1, 100);

    let entries = match kind.as_str() {
        "artists" => {
            sqlx::query_as!(
                ChartEntry,
                r#"WITH span AS (
                       SELECT coalesce($2::date::timestamp AT TIME ZONE $4, '-infinity') AS lo,
                              coalesce(($3::date + 1)::timestamp AT TIME ZONE $4, 'infinity') AS hi
                   )
                   SELECT a.id, a.name, NULL::text AS artist, count(*) AS "listens!"
                     FROM span, listen l
                     JOIN artist a ON a.id = l.artist_id
                    WHERE l.profile_id = $1 AND l.listened_at >= span.lo AND l.listened_at < span.hi
                    GROUP BY a.id
                    ORDER BY count(*) DESC, a.name
                    LIMIT $5"#,
                profile.id,
                from,
                to,
                tz,
                limit,
            )
            .fetch_all(&state.db)
            .await
        }
        "releases" => {
            sqlx::query_as!(
                ChartEntry,
                r#"WITH span AS (
                       SELECT coalesce($2::date::timestamp AT TIME ZONE $4, '-infinity') AS lo,
                              coalesce(($3::date + 1)::timestamp AT TIME ZONE $4, 'infinity') AS hi
                   )
                   SELECT r.id, r.title AS name, a.name AS "artist?", count(*) AS "listens!"
                     FROM span, listen l
                     JOIN release r ON r.id = l.release_id
                     JOIN artist a ON a.id = r.artist_id
                    WHERE l.profile_id = $1 AND l.listened_at >= span.lo AND l.listened_at < span.hi
                    GROUP BY r.id, a.id
                    ORDER BY count(*) DESC, r.title
                    LIMIT $5"#,
                profile.id,
                from,
                to,
                tz,
                limit,
            )
            .fetch_all(&state.db)
            .await
        }
        "recordings" => {
            sqlx::query_as!(
                ChartEntry,
                r#"WITH span AS (
                       SELECT coalesce($2::date::timestamp AT TIME ZONE $4, '-infinity') AS lo,
                              coalesce(($3::date + 1)::timestamp AT TIME ZONE $4, 'infinity') AS hi
                   )
                   SELECT r.id, r.title AS name, a.name AS "artist?", count(*) AS "listens!"
                     FROM span, listen l
                     JOIN recording r ON r.id = l.recording_id
                     JOIN artist a ON a.id = r.artist_id
                    WHERE l.profile_id = $1 AND l.listened_at >= span.lo AND l.listened_at < span.hi
                    GROUP BY r.id, a.id
                    ORDER BY count(*) DESC, r.title
                    LIMIT $5"#,
                profile.id,
                from,
                to,
                tz,
                limit,
            )
            .fetch_all(&state.db)
            .await
        }
        _ => return Err(AppError::NotFound),
    }?;

    Ok(Json(entries))
}

#[derive(Deserialize)]
struct YearsParams {
    tz: Option<String>,
    limit: Option<i64>,
}

#[derive(Serialize)]
struct YearTop {
    year: i32,
    /// All listens of the year, not only those of the artists below.
    listens: i64,
    artists: Vec<ChartEntry>,
}

/// The most heard artists of every year with listens, ranked like the yearly charts,
/// to show how the favourites change over the years.
async fn top_artists_per_year(
    State(state): State<AppState>,
    viewer: Viewer,
    Path((username, slug)): Path<(String, String)>,
    Query(params): Query<YearsParams>,
) -> Result<Json<Vec<YearTop>>, AppError> {
    let profile = find_profile(&state.db, viewer, &username, &slug).await?;
    let tz = time_zone(&state.db, params.tz).await?;
    let limit = params.limit.unwrap_or(10).clamp(1, 100);

    let rows = sqlx::query!(
        r#"WITH counts AS (
               SELECT date_part('year', listened_at AT TIME ZONE $2)::int AS year,
                      artist_id, count(*) AS listens
                 FROM listen
                WHERE profile_id = $1
                GROUP BY 1, 2
           ), ranked AS (
               SELECT c.year, a.id, a.name, c.listens,
                      sum(c.listens) OVER (PARTITION BY c.year)::bigint AS year_listens,
                      row_number() OVER (PARTITION BY c.year ORDER BY c.listens DESC, a.name) AS rank
                 FROM counts c
                 JOIN artist a ON a.id = c.artist_id
           )
           SELECT year AS "year!", year_listens AS "year_listens!", id AS "id!", name AS "name!",
                  listens AS "listens!"
             FROM ranked
            WHERE rank <= $3
            ORDER BY year, rank"#,
        profile.id,
        tz,
        limit,
    )
    .fetch_all(&state.db)
    .await?;

    let mut years: Vec<YearTop> = Vec::new();
    for row in rows {
        let artist = ChartEntry {
            id: row.id,
            name: row.name,
            artist: None,
            listens: row.listens,
        };
        match years.last_mut() {
            Some(year) if year.year == row.year => year.artists.push(artist),
            _ => years.push(YearTop {
                year: row.year,
                listens: row.year_listens,
                artists: vec![artist],
            }),
        }
    }
    Ok(Json(years))
}

#[derive(Deserialize)]
struct ListensParams {
    /// Only listens strictly before this time; pass a page's `next` to get the following page.
    #[serde(default, with = "time::serde::rfc3339::option")]
    before: Option<OffsetDateTime>,
    limit: Option<i64>,
}

#[derive(Serialize)]
struct ListensPage {
    listens: Vec<ListenEntry>,
    /// Set when there are older listens.
    #[serde(with = "time::serde::rfc3339::option")]
    next: Option<OffsetDateTime>,
}

#[derive(Serialize)]
struct ListenEntry {
    #[serde(with = "time::serde::rfc3339")]
    listened_at: OffsetDateTime,
    artist: String,
    track: String,
    album: Option<String>,
    artist_id: i64,
    recording_id: i64,
    release_id: Option<i64>,
}

async fn listens(
    State(state): State<AppState>,
    viewer: Viewer,
    Path((username, slug)): Path<(String, String)>,
    Query(params): Query<ListensParams>,
) -> Result<Json<ListensPage>, AppError> {
    let profile = find_profile(&state.db, viewer, &username, &slug).await?;
    let limit = params.limit.unwrap_or(50).clamp(1, 200);

    // One row more than asked for tells whether there is another page.
    let mut listens = sqlx::query_as!(
        ListenEntry,
        r#"SELECT l.listened_at, a.name AS artist, r.title AS track, rel.title AS "album?",
                  l.artist_id, l.recording_id, l.release_id
             FROM listen l
             JOIN artist a ON a.id = l.artist_id
             JOIN recording r ON r.id = l.recording_id
             LEFT JOIN release rel ON rel.id = l.release_id
            WHERE l.profile_id = $1 AND l.listened_at < coalesce($2::timestamptz, 'infinity')
            ORDER BY l.listened_at DESC
            LIMIT $3"#,
        profile.id,
        params.before,
        limit + 1,
    )
    .fetch_all(&state.db)
    .await?;

    let next = if listens.len() as i64 > limit {
        listens.truncate(limit as usize);
        listens.last().map(|l| l.listened_at)
    } else {
        None
    };
    Ok(Json(ListensPage { listens, next }))
}

#[derive(Serialize)]
struct NowPlaying {
    artist: String,
    track: String,
    album: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    started_at: OffsetDateTime,
    duration_ms: Option<i32>,
}

/// The track a client reported as playing, or `null` once it has run out.
async fn now_playing(
    State(state): State<AppState>,
    viewer: Viewer,
    Path((username, slug)): Path<(String, String)>,
) -> Result<Json<Option<NowPlaying>>, AppError> {
    let profile = find_profile(&state.db, viewer, &username, &slug).await?;
    let playing = sqlx::query_as!(
        NowPlaying,
        "SELECT artist_raw AS artist, track_raw AS track, album_raw AS album, started_at, duration_ms
           FROM now_playing
          WHERE profile_id = $1 AND expires_at > now()",
        profile.id,
    )
    .fetch_optional(&state.db)
    .await?;
    Ok(Json(playing))
}

/// The time zone for year boundaries, UTC by default. PostgreSQL knows the names
/// (e.g. Europe/Berlin) and rejects unknown ones with invalid_parameter_value.
pub(crate) async fn time_zone(db: &PgPool, tz: Option<String>) -> Result<String, AppError> {
    let Some(tz) = tz else {
        return Ok("UTC".into());
    };
    match sqlx::query_scalar!("SELECT now() AT TIME ZONE $1", tz)
        .fetch_one(db)
        .await
    {
        Ok(_) => Ok(tz),
        Err(sqlx::Error::Database(err)) if err.code().as_deref() == Some("22023") => {
            Err(AppError::BadRequest(format!("unknown time zone: {tz}")))
        }
        Err(err) => Err(err.into()),
    }
}
