//! Read-only API behind the public profile pages: overview, charts and recent listens.
//!
//! Only public profiles are served; a private one answers 404 like an unknown one
//! until there is a login.

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::get,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use time::OffsetDateTime;

use crate::{AppError, AppState};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/profiles", get(list))
        .route("/profiles/{username}/{slug}", get(overview))
        .route("/profiles/{username}/{slug}/top/{kind}", get(top))
        .route("/profiles/{username}/{slug}/listens", get(listens))
}

#[derive(Serialize)]
struct ProfileSummary {
    username: String,
    slug: String,
    name: String,
    listens: i64,
}

async fn list(State(state): State<AppState>) -> Result<Json<Vec<ProfileSummary>>, AppError> {
    let profiles = sqlx::query_as!(
        ProfileSummary,
        r#"SELECT a.username::text AS "username!", p.slug::text AS "slug!", p.name,
                  (SELECT count(*) FROM listen l WHERE l.profile_id = p.id) AS "listens!"
             FROM profile p
             JOIN account a ON a.id = p.account_id
            WHERE p.visibility = 'public'
            ORDER BY a.username, p.slug"#
    )
    .fetch_all(&state.db)
    .await?;
    Ok(Json(profiles))
}

struct Profile {
    id: i64,
    username: String,
    slug: String,
    name: String,
}

async fn find_profile(db: &PgPool, username: &str, slug: &str) -> Result<Profile, AppError> {
    // Cast the parameters to citext: comparing citext to text would compare case-sensitively.
    sqlx::query_as!(
        Profile,
        r#"SELECT p.id, a.username::text AS "username!", p.slug::text AS "slug!", p.name
             FROM profile p
             JOIN account a ON a.id = p.account_id
            WHERE a.username = $1::text::citext
              AND p.slug = $2::text::citext
              AND p.visibility = 'public'"#,
        username,
        slug,
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
    Path((username, slug)): Path<(String, String)>,
    Query(params): Query<OverviewParams>,
) -> Result<Json<Overview>, AppError> {
    let profile = find_profile(&state.db, &username, &slug).await?;
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
        listens: years.iter().map(|y| y.listens).sum(),
        first_listened_at: span.first,
        last_listened_at: span.last,
        years,
    }))
}

#[derive(Deserialize)]
struct TopParams {
    /// Calendar year in `tz`; all time when missing.
    year: Option<i32>,
    tz: Option<String>,
    limit: Option<i64>,
}

#[derive(Serialize)]
struct ChartEntry {
    id: i64,
    name: String,
    /// The artist of a release or recording; missing in the artist chart.
    #[serde(skip_serializing_if = "Option::is_none")]
    artist: Option<String>,
    listens: i64,
}

// All three charts count listens in [lo, hi): the given year in the given time
// zone, or everything when the year is NULL.
async fn top(
    State(state): State<AppState>,
    Path((username, slug, kind)): Path<(String, String, String)>,
    Query(params): Query<TopParams>,
) -> Result<Json<Vec<ChartEntry>>, AppError> {
    if params.year.is_some_and(|y| !(1..=9999).contains(&y)) {
        return Err(AppError::BadRequest(
            "year must be between 1 and 9999".into(),
        ));
    }
    let profile = find_profile(&state.db, &username, &slug).await?;
    let tz = time_zone(&state.db, params.tz).await?;
    let limit = params.limit.unwrap_or(10).clamp(1, 100);

    let entries = match kind.as_str() {
        "artists" => {
            sqlx::query_as!(
                ChartEntry,
                r#"WITH span AS (
                       SELECT coalesce(make_timestamptz($2, 1, 1, 0, 0, 0, $3), '-infinity') AS lo,
                              coalesce(make_timestamptz($2 + 1, 1, 1, 0, 0, 0, $3), 'infinity') AS hi
                   )
                   SELECT a.id, a.name, NULL::text AS artist, count(*) AS "listens!"
                     FROM span, listen l
                     JOIN artist a ON a.id = l.artist_id
                    WHERE l.profile_id = $1 AND l.listened_at >= span.lo AND l.listened_at < span.hi
                    GROUP BY a.id
                    ORDER BY count(*) DESC, a.name
                    LIMIT $4"#,
                profile.id,
                params.year,
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
                       SELECT coalesce(make_timestamptz($2, 1, 1, 0, 0, 0, $3), '-infinity') AS lo,
                              coalesce(make_timestamptz($2 + 1, 1, 1, 0, 0, 0, $3), 'infinity') AS hi
                   )
                   SELECT r.id, r.title AS name, a.name AS "artist?", count(*) AS "listens!"
                     FROM span, listen l
                     JOIN release r ON r.id = l.release_id
                     JOIN artist a ON a.id = r.artist_id
                    WHERE l.profile_id = $1 AND l.listened_at >= span.lo AND l.listened_at < span.hi
                    GROUP BY r.id, a.id
                    ORDER BY count(*) DESC, r.title
                    LIMIT $4"#,
                profile.id,
                params.year,
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
                       SELECT coalesce(make_timestamptz($2, 1, 1, 0, 0, 0, $3), '-infinity') AS lo,
                              coalesce(make_timestamptz($2 + 1, 1, 1, 0, 0, 0, $3), 'infinity') AS hi
                   )
                   SELECT r.id, r.title AS name, a.name AS "artist?", count(*) AS "listens!"
                     FROM span, listen l
                     JOIN recording r ON r.id = l.recording_id
                     JOIN artist a ON a.id = r.artist_id
                    WHERE l.profile_id = $1 AND l.listened_at >= span.lo AND l.listened_at < span.hi
                    GROUP BY r.id, a.id
                    ORDER BY count(*) DESC, r.title
                    LIMIT $4"#,
                profile.id,
                params.year,
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
}

async fn listens(
    State(state): State<AppState>,
    Path((username, slug)): Path<(String, String)>,
    Query(params): Query<ListensParams>,
) -> Result<Json<ListensPage>, AppError> {
    let profile = find_profile(&state.db, &username, &slug).await?;
    let limit = params.limit.unwrap_or(50).clamp(1, 200);

    // One row more than asked for tells whether there is another page.
    let mut listens = sqlx::query_as!(
        ListenEntry,
        r#"SELECT l.listened_at, a.name AS artist, r.title AS track, rel.title AS "album?"
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

/// The time zone for year boundaries, UTC by default. PostgreSQL knows the names
/// (e.g. Europe/Berlin) and rejects unknown ones with invalid_parameter_value.
async fn time_zone(db: &PgPool, tz: Option<String>) -> Result<String, AppError> {
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
