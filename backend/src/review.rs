//! A year in review for a profile: listens against the year before, the artists
//! heard for the first time, the biggest risers, the longest listening session
//! and the listens per month.
//!
//! The year runs from midnight on 1 January to midnight on the next one in the
//! viewer's time zone. While it is still running it counts up to now and is
//! compared with the year before up to the same day and time, like the week at
//! a glance in [`crate::week`].

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::get,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use time::{Date, OffsetDateTime};

use crate::{
    AppError, AppState,
    account::Viewer,
    profiles::{find_profile, time_zone},
};

time::serde::format_description!(day, Date, "[year]-[month]-[day]");

pub fn routes() -> Router<AppState> {
    Router::new().route("/profiles/{username}/{slug}/year/{year}", get(review))
}

/// How long a pause between two listens may be without ending the session:
/// counted from the end of the earlier track, or from its start when its length
/// is unknown.
const SESSION_PAUSE: &str = "30 minutes";

#[derive(Deserialize)]
struct ReviewParams {
    tz: Option<String>,
    /// Only the listens of this source, see `listen_source` in the migrations.
    source: Option<String>,
}

#[derive(Serialize)]
struct Review {
    year: i32,
    /// False while the year is still running; it then counts up to now.
    complete: bool,
    listens: i64,
    /// The year before up to the same day and time; all of it for a past year.
    last_year_so_far: i64,
    /// The whole year before.
    last_year: i64,
    artists: i64,
    recordings: i64,
    /// Days with listens.
    days: i64,
    /// Artists of the year the profile had never heard before it.
    new_artists: i64,
    /// The most heard of them.
    discoveries: Vec<Discovery>,
    /// Artists heard before the year whose listens grew the most against the
    /// year before (up to the same day and time).
    risers: Vec<Riser>,
    longest_session: Option<Session>,
    /// January to December, each with the listens of the same month the year before.
    months: Vec<MonthCount>,
    /// The day with the most listens, the earliest one if there are several.
    top_day: Option<DayCount>,
}

#[derive(Serialize)]
struct Discovery {
    id: i64,
    name: String,
    listens: i64,
    #[serde(with = "time::serde::rfc3339")]
    first_listened_at: OffsetDateTime,
}

#[derive(Serialize)]
struct Riser {
    id: i64,
    name: String,
    listens: i64,
    last_year: i64,
    /// Places in the artist charts of the year and of the year before; `None`
    /// when not heard in the year before.
    rank: i64,
    last_rank: Option<i64>,
}

#[derive(Serialize)]
struct Session {
    #[serde(with = "time::serde::rfc3339")]
    started_at: OffsetDateTime,
    /// The end of the last track, or its start when its length is unknown.
    #[serde(with = "time::serde::rfc3339")]
    ended_at: OffsetDateTime,
    listens: i64,
    /// The most heard artists of the session.
    artists: Vec<SessionArtist>,
}

#[derive(Serialize)]
struct SessionArtist {
    id: i64,
    name: String,
    listens: i64,
}

#[derive(Serialize)]
struct MonthCount {
    month: i32,
    listens: i64,
    last_year: i64,
}

#[derive(Serialize)]
struct DayCount {
    #[serde(with = "day")]
    date: Date,
    listens: i64,
}

/// The year is [lo, cut) and the one before [prev_lo, prev_cut), up to the same
/// day and time; `cut` is now while the year runs and `hi` otherwise.
struct Bounds {
    lo: OffsetDateTime,
    cut: OffsetDateTime,
    hi: OffsetDateTime,
    prev_lo: OffsetDateTime,
    prev_cut: OffsetDateTime,
}

async fn bounds(db: &PgPool, year: i32, tz: &str) -> Result<Bounds, AppError> {
    Ok(sqlx::query_as!(
        Bounds,
        r#"SELECT lo AS "lo!", cut AS "cut!", hi AS "hi!", prev_lo AS "prev_lo!",
                  ((cut AT TIME ZONE $2) - interval '1 year') AT TIME ZONE $2 AS "prev_cut!"
             FROM (SELECT make_date($1, 1, 1)::timestamp AT TIME ZONE $2 AS lo,
                          (make_date($1, 1, 1) + interval '1 year') AT TIME ZONE $2 AS hi,
                          (make_date($1, 1, 1) - interval '1 year') AT TIME ZONE $2 AS prev_lo) b,
                  LATERAL (SELECT greatest(lo, least(now(), hi)) AS cut) c"#,
        year,
        tz,
    )
    .fetch_one(db)
    .await?)
}

async fn review(
    State(state): State<AppState>,
    viewer: Viewer,
    Path((username, slug, year)): Path<(String, String, i32)>,
    Query(params): Query<ReviewParams>,
) -> Result<Json<Review>, AppError> {
    if !(1..=9999).contains(&year) {
        return Err(AppError::BadRequest(
            "year must be between 1 and 9999".into(),
        ));
    }
    let profile = find_profile(&state.db, viewer, &username, &slug).await?;
    let tz = time_zone(&state.db, params.tz).await?;
    let db = &state.db;
    let source = params.source;
    let b = bounds(db, year, &tz).await?;

    let totals = sqlx::query!(
        r#"SELECT count(*) FILTER (WHERE listened_at >= $2) AS "listens!",
                  count(*) FILTER (WHERE listened_at < $5) AS "last_year_so_far!",
                  count(*) FILTER (WHERE listened_at < $2) AS "last_year!",
                  count(DISTINCT artist_id) FILTER (WHERE listened_at >= $2) AS "artists!",
                  count(DISTINCT recording_id) FILTER (WHERE listened_at >= $2) AS "recordings!",
                  count(DISTINCT (listened_at AT TIME ZONE $6)::date)
                      FILTER (WHERE listened_at >= $2) AS "days!"
             FROM listen
            WHERE profile_id = $1 AND listened_at >= $4 AND listened_at < $3
              AND ($7::text IS NULL OR listen_source(client) = $7)"#,
        profile.id,
        b.lo,
        b.cut,
        b.prev_lo,
        b.prev_cut,
        tz,
        source,
    )
    .fetch_one(db)
    .await?;

    let discovered = sqlx::query!(
        r#"WITH heard AS (
               SELECT artist_id, count(*) AS listens, min(listened_at) AS first
                 FROM listen
                WHERE profile_id = $1 AND listened_at >= $2 AND listened_at < $3
                  AND ($4::text IS NULL OR listen_source(client) = $4)
                GROUP BY 1
           ), new AS (
               SELECT h.*
                 FROM heard h
                WHERE NOT EXISTS (SELECT FROM listen e
                                   WHERE e.profile_id = $1 AND e.artist_id = h.artist_id
                                     AND e.listened_at < $2
                                     AND ($4::text IS NULL OR listen_source(e.client) = $4))
           )
           SELECT a.id, a.name, n.listens AS "listens!", n.first AS "first!",
                  count(*) OVER () AS "total!"
             FROM new n
             JOIN artist a ON a.id = n.artist_id
            ORDER BY n.listens DESC, n.first
            LIMIT 10"#,
        profile.id,
        b.lo,
        b.cut,
        source,
    )
    .fetch_all(db)
    .await?;
    let new_artists = discovered.first().map_or(0, |d| d.total);
    let discoveries = discovered
        .into_iter()
        .map(|d| Discovery {
            id: d.id,
            name: d.name,
            listens: d.listens,
            first_listened_at: d.first,
        })
        .collect();

    // Ranked like the artist charts: most listens first, then by name.
    let risers = sqlx::query_as!(
        Riser,
        r#"WITH this AS (
               SELECT l.artist_id, count(*) AS listens,
                      row_number() OVER (ORDER BY count(*) DESC, min(a.name)) AS rank
                 FROM listen l
                 JOIN artist a ON a.id = l.artist_id
                WHERE l.profile_id = $1 AND l.listened_at >= $2 AND l.listened_at < $3
                  AND ($6::text IS NULL OR listen_source(l.client) = $6)
                GROUP BY 1
           ), last AS (
               SELECT l.artist_id, count(*) AS listens,
                      row_number() OVER (ORDER BY count(*) DESC, min(a.name)) AS rank
                 FROM listen l
                 JOIN artist a ON a.id = l.artist_id
                WHERE l.profile_id = $1 AND l.listened_at >= $4 AND l.listened_at < $5
                  AND ($6::text IS NULL OR listen_source(l.client) = $6)
                GROUP BY 1
           )
           SELECT a.id, a.name, t.listens AS "listens!", coalesce(p.listens, 0) AS "last_year!",
                  t.rank AS "rank!", p.rank AS "last_rank?"
             FROM this t
             JOIN artist a ON a.id = t.artist_id
             LEFT JOIN last p ON p.artist_id = t.artist_id
            WHERE t.listens > coalesce(p.listens, 0)
              AND EXISTS (SELECT FROM listen e
                           WHERE e.profile_id = $1 AND e.artist_id = t.artist_id
                             AND e.listened_at < $2
                             AND ($6::text IS NULL OR listen_source(e.client) = $6))
            ORDER BY t.listens - coalesce(p.listens, 0) DESC, t.rank
            LIMIT 10"#,
        profile.id,
        b.lo,
        b.cut,
        b.prev_lo,
        b.prev_cut,
        source,
    )
    .fetch_all(db)
    .await?;

    // A listen starts a new session when it comes more than SESSION_PAUSE after
    // the end of the one before. Track lengths over three hours are taken for
    // three hours, so that a broken length doesn't glue a day together.
    let session = sqlx::query!(
        r#"WITH heard AS (
               SELECT l.listened_at,
                      l.listened_at + least(interval '3 hours',
                          coalesce(l.duration_ms, r.length_ms, 0) * interval '1 millisecond')
                          AS ended_at
                 FROM listen l
                 JOIN recording r ON r.id = l.recording_id
                WHERE l.profile_id = $1 AND l.listened_at >= $2 AND l.listened_at < $3
                  AND ($4::text IS NULL OR listen_source(l.client) = $4)
           ), marked AS (
               SELECT *, CASE WHEN listened_at > lag(ended_at) OVER (ORDER BY listened_at)
                                                 + $5::text::interval
                              THEN 1 ELSE 0 END AS starts
                 FROM heard
           ), numbered AS (
               SELECT *, sum(starts) OVER (ORDER BY listened_at) AS session FROM marked
           )
           SELECT min(listened_at) AS "started_at!", max(listened_at) AS "last_at!",
                  max(ended_at) AS "ended_at!", count(*) AS "listens!"
             FROM numbered
            GROUP BY session
            ORDER BY max(ended_at) - min(listened_at) DESC, count(*) DESC, min(listened_at)
            LIMIT 1"#,
        profile.id,
        b.lo,
        b.cut,
        source,
        SESSION_PAUSE,
    )
    .fetch_optional(db)
    .await?;

    let longest_session = match session {
        None => None,
        Some(s) => {
            let artists = sqlx::query_as!(
                SessionArtist,
                r#"SELECT a.id, a.name, count(*) AS "listens!"
                     FROM listen l
                     JOIN artist a ON a.id = l.artist_id
                    WHERE l.profile_id = $1 AND l.listened_at >= $2 AND l.listened_at <= $3
                      AND ($4::text IS NULL OR listen_source(l.client) = $4)
                    GROUP BY a.id
                    ORDER BY count(*) DESC, a.name
                    LIMIT 3"#,
                profile.id,
                s.started_at,
                s.last_at,
                source,
            )
            .fetch_all(db)
            .await?;
            Some(Session {
                started_at: s.started_at,
                ended_at: s.ended_at,
                listens: s.listens,
                artists,
            })
        }
    };

    // The months of the year before count all of it, also while this one runs.
    let per_month = sqlx::query!(
        r#"SELECT date_part('month', listened_at AT TIME ZONE $4)::int AS "month!",
                  count(*) FILTER (WHERE listened_at >= $3) AS "listens!",
                  count(*) FILTER (WHERE listened_at < $3) AS "last_year!"
             FROM listen
            WHERE profile_id = $1 AND listened_at >= $2 AND listened_at < $5
              AND ($6::text IS NULL OR listen_source(client) = $6)
            GROUP BY 1"#,
        profile.id,
        b.prev_lo,
        b.lo,
        tz,
        b.cut,
        source,
    )
    .fetch_all(db)
    .await?;
    let months = (1..=12)
        .map(|month| {
            let row = per_month.iter().find(|row| row.month == month);
            MonthCount {
                month,
                listens: row.map_or(0, |row| row.listens),
                last_year: row.map_or(0, |row| row.last_year),
            }
        })
        .collect();

    let top_day = sqlx::query_as!(
        DayCount,
        r#"SELECT (listened_at AT TIME ZONE $4)::date AS "date!", count(*) AS "listens!"
             FROM listen
            WHERE profile_id = $1 AND listened_at >= $2 AND listened_at < $3
              AND ($5::text IS NULL OR listen_source(client) = $5)
            GROUP BY 1
            ORDER BY 2 DESC, 1
            LIMIT 1"#,
        profile.id,
        b.lo,
        b.cut,
        tz,
        source,
    )
    .fetch_optional(db)
    .await?;

    Ok(Json(Review {
        year,
        complete: b.cut >= b.hi,
        listens: totals.listens,
        last_year_so_far: totals.last_year_so_far,
        last_year: totals.last_year,
        artists: totals.artists,
        recordings: totals.recordings,
        days: totals.days,
        new_artists,
        discoveries,
        risers,
        longest_session,
        months,
        top_day,
    }))
}
