//! "On this day" on a profile page: what was heard on today's date in earlier
//! years, one entry per year that has listens on it, the latest year first.
//!
//! Days run from midnight to midnight in the viewer's time zone. In a year
//! without 29 February, 29 February looks back to the 28th.

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::get,
};
use serde::{Deserialize, Serialize};
use time::Date;

use crate::{
    AppError, AppState,
    account::Viewer,
    profiles::{ChartEntry, find_profile, time_zone},
};

time::serde::format_description!(day, Date, "[year]-[month]-[day]");

/// The most heard artists and tracks shown per year.
const ARTISTS: i64 = 3;
const TRACKS: i64 = 5;

pub fn routes() -> Router<AppState> {
    Router::new().route("/profiles/{username}/{slug}/on-this-day", get(on_this_day))
}

#[derive(Deserialize)]
struct Params {
    /// The day to look back from, in `tz`; today when missing.
    #[serde(default, with = "day::option")]
    day: Option<Date>,
    tz: Option<String>,
    /// Only the listens of this source, see `listen_source` in the migrations.
    source: Option<String>,
}

#[derive(Serialize)]
struct OnThisDay {
    #[serde(with = "day")]
    day: Date,
    years: Vec<Year>,
}

#[derive(Serialize)]
struct Year {
    #[serde(with = "day")]
    date: Date,
    years_ago: i32,
    listens: i64,
    artists: Vec<ChartEntry>,
    tracks: Vec<ChartEntry>,
}

async fn on_this_day(
    State(state): State<AppState>,
    viewer: Viewer,
    Path((username, slug)): Path<(String, String)>,
    Query(params): Query<Params>,
) -> Result<Json<OnThisDay>, AppError> {
    if params.day.is_some_and(|d| !(1..=9999).contains(&d.year())) {
        return Err(AppError::BadRequest(
            "day must be between the years 1 and 9999".into(),
        ));
    }
    let profile = find_profile(&state.db, viewer, &username, &slug).await?;
    let tz = time_zone(&state.db, params.tz).await?;
    let db = &state.db;

    let day = match params.day {
        Some(day) => day,
        None => {
            sqlx::query_scalar!(r#"SELECT (now() AT TIME ZONE $1)::date AS "day!""#, tz)
                .fetch_one(db)
                .await?
        }
    };

    // One row per year with its listens, then its top artists and tracks. The
    // years go back to the first listen, each day looked up through the index.
    let rows = sqlx::query!(
        r#"WITH days AS (
               SELECT n, ($2::date - make_interval(years => n))::date AS d
                 FROM generate_series(1, extract(year FROM $2::date)::int - (
                          SELECT extract(year FROM min(listened_at) AT TIME ZONE $3)::int
                            FROM listen WHERE profile_id = $1)) n
           ), heard AS MATERIALIZED (
               SELECT d.n, d.d, l.artist_id, l.recording_id
                 FROM days d
                 JOIN listen l ON l.profile_id = $1
                  AND l.listened_at >= d.d::timestamp AT TIME ZONE $3
                  AND l.listened_at < (d.d + 1)::timestamp AT TIME ZONE $3
                WHERE ($4::text IS NULL OR listen_source(l.client) = $4)
           ), artists AS (
               SELECT h.n, a.id, a.name, count(*) AS listens,
                      row_number() OVER (PARTITION BY h.n ORDER BY count(*) DESC, a.name) AS rank
                 FROM heard h JOIN artist a ON a.id = h.artist_id
                GROUP BY h.n, a.id
           ), tracks AS (
               SELECT h.n, r.id, r.title AS name, a.name AS artist, count(*) AS listens,
                      row_number() OVER (PARTITION BY h.n ORDER BY count(*) DESC, r.title) AS rank
                 FROM heard h
                 JOIN recording r ON r.id = h.recording_id
                 JOIN artist a ON a.id = r.artist_id
                GROUP BY h.n, r.id, a.id
           )
           SELECT 'year' AS "kind!", n AS "n!", d AS date, 0::bigint AS "id!", ''::text AS "name!",
                  NULL::text AS artist, count(*) AS "listens!"
             FROM heard GROUP BY n, d
           UNION ALL
           SELECT 'artist', n, NULL, id, name, NULL, listens FROM artists WHERE rank <= $5
           UNION ALL
           SELECT 'track', n, NULL, id, name, artist, listens FROM tracks WHERE rank <= $6
           ORDER BY 2, 1 DESC, 7 DESC, 5"#,
        profile.id,
        day,
        tz,
        params.source,
        ARTISTS,
        TRACKS,
    )
    .fetch_all(db)
    .await?;

    // Rows come by year (latest first), each year's own row before its entries.
    let mut years: Vec<Year> = Vec::new();
    for row in rows {
        let entry = || ChartEntry {
            id: row.id,
            name: row.name.clone(),
            artist: row.artist.clone(),
            listens: row.listens,
        };
        match row.kind.as_str() {
            "year" => years.push(Year {
                date: row.date.unwrap_or(day),
                years_ago: row.n,
                listens: row.listens,
                artists: Vec::new(),
                tracks: Vec::new(),
            }),
            "artist" => years.last_mut().unwrap().artists.push(entry()),
            _ => years.last_mut().unwrap().tracks.push(entry()),
        }
    }

    Ok(Json(OnThisDay { day, years }))
}
