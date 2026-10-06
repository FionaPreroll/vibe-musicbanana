//! The week at a glance for the top of a profile page: listens per day against
//! the week before, the artists of the week and which of them are new, and the
//! streak of days in a row with listens.
//!
//! Weeks run from Monday to Sunday in the viewer's time zone, or from the day
//! the viewer starts them on to the day before it. The current week
//! is compared with the week before up to the same weekday and time of day, so
//! a Wednesday morning isn't held against a whole week.

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::get,
};
use serde::{Deserialize, Serialize};
use time::{Date, Duration};

use crate::{
    AppError, AppState,
    account::Viewer,
    profiles::{find_profile, time_zone},
};

time::serde::format_description!(day, Date, "[year]-[month]-[day]");

pub fn routes() -> Router<AppState> {
    Router::new().route("/profiles/{username}/{slug}/week", get(week))
}

#[derive(Deserialize)]
struct WeekParams {
    /// The day to look from, in `tz`; today when missing. The week is the one
    /// it falls in, counted up to its end (or up to now, for today).
    #[serde(default, with = "day::option")]
    day: Option<Date>,
    tz: Option<String>,
    /// The first day of the week, 1 for Monday (the default) to 7 for Sunday.
    week_start: Option<u8>,
    /// Only the listens of this source, see `listen_source` in the migrations.
    source: Option<String>,
}

#[derive(Serialize)]
struct Week {
    /// The first and last day of the week.
    #[serde(with = "day")]
    from: Date,
    #[serde(with = "day")]
    to: Date,
    /// The day looked from; the days after it have no listens yet.
    #[serde(with = "day")]
    day: Date,
    /// The seven days of the week, each with the listens on the same weekday of the week before.
    days: Vec<DayCount>,
    /// The listens of the week so far.
    listens: i64,
    /// The week before up to the same weekday and time of day.
    last_week_so_far: i64,
    /// The whole week before.
    last_week: i64,
    /// The most heard artists of the week so far.
    artists: Vec<WeekArtist>,
    /// How many artists of the week the profile had never heard before it.
    new_artists: i64,
    streak: Streak,
}

#[derive(Serialize)]
struct DayCount {
    #[serde(with = "day")]
    date: Date,
    listens: i64,
    last_week: i64,
}

#[derive(Serialize)]
struct WeekArtist {
    id: i64,
    name: String,
    listens: i64,
    /// Listens in the whole week before.
    last_week: i64,
    /// Never heard before this week.
    new: bool,
}

#[derive(Serialize)]
struct Streak {
    /// Days in a row with listens up to the day looked from, or up to the day
    /// before it while that day has none yet; 0 when neither has any.
    current: i64,
    #[serde(with = "day::option")]
    current_from: Option<Date>,
    /// The most days in a row ever, the latest such run if there are several.
    longest: i64,
    #[serde(with = "day::option")]
    longest_from: Option<Date>,
    #[serde(with = "day::option")]
    longest_to: Option<Date>,
}

async fn week(
    State(state): State<AppState>,
    viewer: Viewer,
    Path((username, slug)): Path<(String, String)>,
    Query(params): Query<WeekParams>,
) -> Result<Json<Week>, AppError> {
    if params.day.is_some_and(|d| !(1..=9999).contains(&d.year())) {
        return Err(AppError::BadRequest(
            "day must be between the years 1 and 9999".into(),
        ));
    }
    let week_start = params.week_start.unwrap_or(1);
    if !(1..=7).contains(&week_start) {
        return Err(AppError::BadRequest(
            "week_start must be 1 (Monday) to 7 (Sunday)".into(),
        ));
    }
    let profile = find_profile(&state.db, viewer, &username, &slug).await?;
    let tz = time_zone(&state.db, params.tz).await?;
    let db = &state.db;
    let source = params.source;

    let day = match params.day {
        Some(day) => day,
        None => {
            sqlx::query_scalar!(r#"SELECT (now() AT TIME ZONE $1)::date AS "day!""#, tz)
                .fetch_one(db)
                .await?
        }
    };
    let first =
        day - Duration::days(((day.weekday().number_from_monday() + 7 - week_start) % 7).into());

    // [lo, cut) is the week so far, [last_lo, lo) the week before and
    // [last_lo, last_cut) that one up to the same time. Days in the time zone
    // start at local midnight, also across a change to or from summer time.

    let per_day = sqlx::query!(
        r#"WITH bounds AS (
               SELECT lo, cut, last_lo,
                      ((cut AT TIME ZONE $3) - interval '7 days') AT TIME ZONE $3 AS last_cut
                 FROM (SELECT $2::date::timestamp AT TIME ZONE $3 AS lo,
                              least(now(), ($4::date + 1)::timestamp AT TIME ZONE $3) AS cut,
                              ($2::date - 7)::timestamp AT TIME ZONE $3 AS last_lo) b
           )
           SELECT (l.listened_at AT TIME ZONE $3)::date AS "date!", count(*) AS "listens!",
                  count(*) FILTER (WHERE l.listened_at < b.last_cut) AS "so_far!"
             FROM bounds b, listen l
            WHERE l.profile_id = $1 AND l.listened_at >= b.last_lo
              AND l.listened_at < b.cut
              AND ($5::text IS NULL OR listen_source(l.client) = $5)
            GROUP BY 1"#,
        profile.id,
        first,
        tz,
        day,
        source,
    )
    .fetch_all(db)
    .await?;

    let count_on = |date: Date| {
        per_day
            .iter()
            .find(|row| row.date == date)
            .map_or(0, |row| row.listens)
    };
    let days: Vec<DayCount> = (0..7)
        .map(|i| {
            let date = first + Duration::days(i);
            DayCount {
                date,
                listens: count_on(date),
                last_week: count_on(date - Duration::days(7)),
            }
        })
        .collect();
    let listens = days.iter().map(|d| d.listens).sum();
    let last_week = days.iter().map(|d| d.last_week).sum();
    let last_week_so_far = per_day
        .iter()
        .filter(|row| row.date < first)
        .map(|row| row.so_far)
        .sum();

    let artists = sqlx::query_as!(
        WeekArtist,
        r#"WITH bounds AS (
               SELECT $2::date::timestamp AT TIME ZONE $3 AS lo,
                      least(now(), ($4::date + 1)::timestamp AT TIME ZONE $3) AS cut,
                      ($2::date - 7)::timestamp AT TIME ZONE $3 AS last_lo
           ), heard AS (
               SELECT l.artist_id, count(*) AS listens
                 FROM bounds b, listen l
                WHERE l.profile_id = $1 AND l.listened_at >= b.lo AND l.listened_at < b.cut
                  AND ($5::text IS NULL OR listen_source(l.client) = $5)
                GROUP BY 1
                ORDER BY 2 DESC
           )
           SELECT a.id, a.name, h.listens AS "listens!",
                  (SELECT count(*) FROM listen l
                    WHERE l.profile_id = $1 AND l.artist_id = a.id
                      AND l.listened_at >= b.last_lo AND l.listened_at < b.lo
                      AND ($5::text IS NULL OR listen_source(l.client) = $5)) AS "last_week!",
                  NOT EXISTS (SELECT FROM listen l
                               WHERE l.profile_id = $1 AND l.artist_id = a.id
                                 AND l.listened_at < b.lo
                                 AND ($5::text IS NULL OR listen_source(l.client) = $5)) AS "new!"
             FROM bounds b, heard h
             JOIN artist a ON a.id = h.artist_id
            ORDER BY h.listens DESC, a.name
            LIMIT 5"#,
        profile.id,
        first,
        tz,
        day,
        source,
    )
    .fetch_all(db)
    .await?;

    let new_artists = sqlx::query_scalar!(
        r#"WITH bounds AS (
               SELECT $2::date::timestamp AT TIME ZONE $3 AS lo,
                      least(now(), ($4::date + 1)::timestamp AT TIME ZONE $3) AS cut
           )
           SELECT count(DISTINCT l.artist_id) AS "n!"
             FROM bounds b, listen l
            WHERE l.profile_id = $1 AND l.listened_at >= b.lo AND l.listened_at < b.cut
              AND ($5::text IS NULL OR listen_source(l.client) = $5)
              AND NOT EXISTS (SELECT FROM listen e
                               WHERE e.profile_id = $1 AND e.artist_id = l.artist_id
                                 AND e.listened_at < b.lo
                                 AND ($5::text IS NULL OR listen_source(e.client) = $5))"#,
        profile.id,
        first,
        tz,
        day,
        source,
    )
    .fetch_one(db)
    .await?;

    // Runs of days in a row: a day minus its rank is the same for every day of a run.
    let runs = sqlx::query!(
        r#"WITH days AS (
               SELECT DISTINCT (listened_at AT TIME ZONE $2)::date AS d
                 FROM listen
                WHERE profile_id = $1
                  AND listened_at < least(now(), ($3::date + 1)::timestamp AT TIME ZONE $2)
                  AND ($4::text IS NULL OR listen_source(client) = $4)
           ), runs AS (
               SELECT min(d) AS first, max(d) AS last, count(*) AS days
                 FROM (SELECT d, d - row_number() OVER (ORDER BY d)::int AS run FROM days) r
                GROUP BY run
           )
           (SELECT 'longest' AS "kind!", first AS "first!", last AS "last!", days AS "days!"
              FROM runs ORDER BY days DESC, last DESC LIMIT 1)
           UNION ALL
           (SELECT 'latest', first, last, days FROM runs ORDER BY last DESC LIMIT 1)"#,
        profile.id,
        tz,
        day,
        source,
    )
    .fetch_all(db)
    .await?;

    let mut streak = Streak {
        current: 0,
        current_from: None,
        longest: 0,
        longest_from: None,
        longest_to: None,
    };
    for run in runs {
        if run.kind == "longest" {
            streak.longest = run.days;
            streak.longest_from = Some(run.first);
            streak.longest_to = Some(run.last);
        } else if run.last >= day - Duration::days(1) {
            streak.current = run.days;
            streak.current_from = Some(run.first);
        }
    }

    Ok(Json(Week {
        from: first,
        to: first + Duration::days(6),
        day,
        days,
        listens,
        last_week_so_far,
        last_week,
        artists,
        new_artists,
        streak,
    }))
}
