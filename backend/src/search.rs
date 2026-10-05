//! Searching the artists, albums and tracks a profile has heard.

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::get,
};
use serde::{Deserialize, Serialize};

use crate::{
    AppError, AppState,
    account::Viewer,
    profiles::{ChartEntry, find_profile},
};

pub fn routes() -> Router<AppState> {
    Router::new().route("/profiles/{username}/{slug}/search", get(search))
}

#[derive(Deserialize)]
struct Params {
    q: String,
    /// Hits per kind, 10 by default, at most 50.
    limit: Option<i64>,
}

#[derive(Serialize)]
struct Found {
    artists: Vec<ChartEntry>,
    releases: Vec<ChartEntry>,
    recordings: Vec<ChartEntry>,
}

/// Every word of the query must appear in the name, ignoring case and accents
/// (see `search_name` in the migrations); for albums and tracks it may be in the
/// artist's name instead, so "ärzte unrockbar" finds the track. The most heard
/// come first.
async fn search(
    State(state): State<AppState>,
    viewer: Viewer,
    Path((username, slug)): Path<(String, String)>,
    Query(params): Query<Params>,
) -> Result<Json<Found>, AppError> {
    let profile = find_profile(&state.db, viewer, &username, &slug).await?;
    let limit = params.limit.unwrap_or(10).clamp(1, 50);
    let words: Vec<&str> = params.q.split_whitespace().take(10).collect();
    if words.is_empty() {
        return Ok(Json(Found {
            artists: vec![],
            releases: vec![],
            recordings: vec![],
        }));
    }

    // The matching names first, then their listens in the profile through the
    // listen indexes; joining all of the profile's listens to the names instead
    // takes a while for a big profile.
    let artists = sqlx::query_as!(
        ChartEntry,
        r#"WITH w AS (SELECT search_fold(x) AS w FROM unnest($2::text[]) x)
           SELECT a.id, a.name, NULL::text AS "artist?", count(*) AS "listens!"
             FROM listen l
             JOIN artist a ON a.id = l.artist_id
            WHERE l.profile_id = $1
              AND l.artist_id = ANY(ARRAY(
                      SELECT a.id FROM artist a
                       WHERE NOT EXISTS (SELECT FROM w WHERE strpos(a.search_name, w) = 0)))
            GROUP BY a.id
            ORDER BY count(*) DESC, a.name, a.id
            LIMIT $3"#,
        profile.id,
        &words as &[&str],
        limit,
    )
    .fetch_all(&state.db);

    let releases = sqlx::query_as!(
        ChartEntry,
        r#"WITH w AS (SELECT search_fold(x) AS w FROM unnest($2::text[]) x)
           SELECT r.id, r.title AS name, a.name AS "artist?", count(*) AS "listens!"
             FROM listen l
             JOIN release r ON r.id = l.release_id
             JOIN artist a ON a.id = r.artist_id
            WHERE l.profile_id = $1
              AND l.release_id = ANY(ARRAY(
                      SELECT r.id FROM release r JOIN artist a ON a.id = r.artist_id
                       WHERE NOT EXISTS (SELECT FROM w WHERE strpos(r.search_name, w) = 0
                                                         AND strpos(a.search_name, w) = 0)))
            GROUP BY r.id, a.id
            ORDER BY count(*) DESC, r.title, r.id
            LIMIT $3"#,
        profile.id,
        &words as &[&str],
        limit,
    )
    .fetch_all(&state.db);

    let recordings = sqlx::query_as!(
        ChartEntry,
        r#"WITH w AS (SELECT search_fold(x) AS w FROM unnest($2::text[]) x)
           SELECT r.id, r.title AS name, a.name AS "artist?", count(*) AS "listens!"
             FROM listen l
             JOIN recording r ON r.id = l.recording_id
             JOIN artist a ON a.id = r.artist_id
            WHERE l.profile_id = $1
              AND l.recording_id = ANY(ARRAY(
                      SELECT r.id FROM recording r JOIN artist a ON a.id = r.artist_id
                       WHERE NOT EXISTS (SELECT FROM w WHERE strpos(r.search_name, w) = 0
                                                         AND strpos(a.search_name, w) = 0)))
            GROUP BY r.id, a.id
            ORDER BY count(*) DESC, r.title, r.id
            LIMIT $3"#,
        profile.id,
        &words as &[&str],
        limit,
    )
    .fetch_all(&state.db);

    let (artists, releases, recordings) = tokio::try_join!(artists, releases, recordings)?;
    Ok(Json(Found {
        artists,
        releases,
        recordings,
    }))
}
