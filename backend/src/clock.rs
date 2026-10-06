//! The listening clock of a profile page: listens by weekday and hour of the
//! day in the viewer's time zone, for the same periods as the charts.
//!
//! Hours are the local hours of each listen, so a summer evening at 21:00 and a
//! winter evening at 21:00 land in the same cell across a change of summer time.

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::get,
};
use serde::Serialize;

use crate::{
    AppError, AppState,
    account::Viewer,
    profiles::{TopParams, find_profile, time_zone},
};

pub fn routes() -> Router<AppState> {
    Router::new().route("/profiles/{username}/{slug}/clock", get(clock))
}

#[derive(Serialize)]
struct Clock {
    /// All listens of the period.
    listens: i64,
    /// Monday to Sunday, each with the listens of the hours 0 to 23.
    weekdays: [[i64; 24]; 7],
}

async fn clock(
    State(state): State<AppState>,
    viewer: Viewer,
    Path((username, slug)): Path<(String, String)>,
    Query(params): Query<TopParams>,
) -> Result<Json<Clock>, AppError> {
    let (from, to) = params.days()?;
    let profile = find_profile(&state.db, viewer, &username, &slug).await?;
    let tz = time_zone(&state.db, params.tz).await?;

    // Counts listens in [lo, hi) like the charts in profiles.rs. date_part, not
    // extract: extract computes a numeric, which takes much longer per listen.
    let cells = sqlx::query!(
        r#"WITH span AS (
               SELECT coalesce($2::date::timestamp AT TIME ZONE $4, '-infinity') AS lo,
                      coalesce(($3::date + 1)::timestamp AT TIME ZONE $4, 'infinity') AS hi
           )
           SELECT date_part('isodow', l.listened_at AT TIME ZONE $4)::int AS "weekday!",
                  date_part('hour', l.listened_at AT TIME ZONE $4)::int AS "hour!",
                  count(*) AS "listens!"
             FROM span, listen l
            WHERE l.profile_id = $1 AND l.listened_at >= span.lo AND l.listened_at < span.hi
              AND ($5::text IS NULL OR listen_source(l.client) = $5)
            GROUP BY 1, 2"#,
        profile.id,
        from,
        to,
        tz,
        params.source,
    )
    .fetch_all(&state.db)
    .await?;

    let mut weekdays = [[0; 24]; 7];
    for cell in &cells {
        // isodow is 1 for Monday to 7 for Sunday.
        weekdays[cell.weekday as usize - 1][cell.hour as usize] = cell.listens;
    }
    Ok(Json(Clock {
        listens: cells.iter().map(|c| c.listens).sum(),
        weekdays,
    }))
}
