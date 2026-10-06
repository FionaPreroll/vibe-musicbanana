//! "Lost & found" on a profile page: artists and tracks the profile heard a lot
//! and not for a long time, the most heard first. The owner can dismiss an
//! entry they don't want back, and show it again later (see
//! migrations/0014_lost_and_found.sql).
//!
//! "Not for a long time" counts back from the profile's latest listen, so a
//! profile nobody scrobbles to any more still has its own. A source narrows
//! what was heard a lot, not the latest listen: an artist heard on Spotify in
//! 2019 and in Navidrome last week is not lost, whichever source is picked.

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{delete, get},
};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::{
    AppError, AppState,
    account::{Account, Viewer},
    history::own_profile,
    profiles::find_profile,
};

/// How long since the last listen before an artist or track counts as lost.
const GAP_DAYS: i32 = 365;
/// What counts as heard a lot.
const ARTIST_LISTENS: i64 = 20;
const TRACK_LISTENS: i64 = 10;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/profiles/{username}/{slug}/lost-and-found",
            get(lost_and_found),
        )
        .route(
            "/me/profiles/{slug}/lost-and-found/hidden",
            get(hidden).post(hide),
        )
        .route(
            "/me/profiles/{slug}/lost-and-found/hidden/{kind}/{id}",
            delete(unhide),
        )
}

#[derive(Deserialize)]
struct Params {
    limit: Option<i64>,
    /// Only the listens of this source count, see `listen_source` in the migrations.
    source: Option<String>,
}

#[derive(Serialize)]
struct LostAndFound {
    /// Lost means not heard since then: a year before the latest listen.
    #[serde(with = "time::serde::rfc3339::option")]
    since: Option<OffsetDateTime>,
    artists: Vec<Entry>,
    tracks: Vec<Entry>,
    /// How many entries the owner dismissed; only for the owner.
    #[serde(skip_serializing_if = "Option::is_none")]
    hidden: Option<i64>,
}

#[derive(Serialize)]
struct Entry {
    id: i64,
    name: String,
    /// The artist of a track.
    #[serde(skip_serializing_if = "Option::is_none")]
    artist: Option<String>,
    listens: i64,
    #[serde(with = "time::serde::rfc3339")]
    first_listened_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    last_listened_at: OffsetDateTime,
}

async fn lost_and_found(
    State(state): State<AppState>,
    viewer: Viewer,
    Path((username, slug)): Path<(String, String)>,
    Query(params): Query<Params>,
) -> Result<Json<LostAndFound>, AppError> {
    let profile = find_profile(&state.db, viewer, &username, &slug).await?;
    let limit = params.limit.unwrap_or(10).clamp(1, 50);
    let db = &state.db;

    let since = sqlx::query_scalar!(
        "SELECT max(listened_at) - make_interval(days => $2) FROM listen WHERE profile_id = $1",
        profile.id,
        GAP_DAYS,
    )
    .fetch_one(db)
    .await?;
    let Some(since) = since else {
        return Ok(Json(LostAndFound {
            since: None,
            artists: Vec::new(),
            tracks: Vec::new(),
            hidden: profile.own.then_some(0),
        }));
    };

    // A dismissal counts for the entry it was merged into, too.
    let artists = sqlx::query_as!(
        Entry,
        r#"WITH heard AS (
               SELECT artist_id AS id,
                      count(*) FILTER (WHERE $3::text IS NULL OR listen_source(client) = $3)
                          AS listens,
                      min(listened_at) AS first, max(listened_at) AS last
                 FROM listen
                WHERE profile_id = $1
                GROUP BY artist_id
           )
           SELECT a.id, a.name, NULL::text AS artist, h.listens AS "listens!",
                  h.first AS "first_listened_at!", h.last AS "last_listened_at!"
             FROM heard h
             JOIN artist a ON a.id = h.id
            WHERE h.listens >= $4 AND h.last < $2
              AND NOT EXISTS (
                    SELECT FROM lost_found_dismissal d
                      JOIN artist x ON x.id = d.entity_id
                     WHERE d.profile_id = $1 AND d.kind = 'artist'
                       AND coalesce(x.merged_into, x.id) = a.id)
            ORDER BY h.listens DESC, a.name
            LIMIT $5"#,
        profile.id,
        since,
        params.source,
        ARTIST_LISTENS,
        limit,
    )
    .fetch_all(db)
    .await?;

    // A dismissed artist takes its tracks along.
    let tracks = sqlx::query_as!(
        Entry,
        r#"WITH heard AS (
               SELECT recording_id AS id,
                      count(*) FILTER (WHERE $3::text IS NULL OR listen_source(client) = $3)
                          AS listens,
                      min(listened_at) AS first, max(listened_at) AS last
                 FROM listen
                WHERE profile_id = $1
                GROUP BY recording_id
           )
           SELECT r.id, r.title AS name, a.name AS "artist?", h.listens AS "listens!",
                  h.first AS "first_listened_at!", h.last AS "last_listened_at!"
             FROM heard h
             JOIN recording r ON r.id = h.id
             JOIN artist a ON a.id = r.artist_id
            WHERE h.listens >= $4 AND h.last < $2
              AND NOT EXISTS (
                    SELECT FROM lost_found_dismissal d
                      JOIN recording x ON x.id = d.entity_id
                     WHERE d.profile_id = $1 AND d.kind = 'recording'
                       AND coalesce(x.merged_into, x.id) = r.id)
              AND NOT EXISTS (
                    SELECT FROM lost_found_dismissal d
                      JOIN artist x ON x.id = d.entity_id
                     WHERE d.profile_id = $1 AND d.kind = 'artist'
                       AND coalesce(x.merged_into, x.id) = r.artist_id)
            ORDER BY h.listens DESC, r.title
            LIMIT $5"#,
        profile.id,
        since,
        params.source,
        TRACK_LISTENS,
        limit,
    )
    .fetch_all(db)
    .await?;

    let hidden = if profile.own {
        Some(
            sqlx::query_scalar!(
                r#"SELECT count(*) AS "n!" FROM lost_found_dismissal WHERE profile_id = $1"#,
                profile.id,
            )
            .fetch_one(db)
            .await?,
        )
    } else {
        None
    };

    Ok(Json(LostAndFound {
        since: Some(since),
        artists,
        tracks,
        hidden,
    }))
}

/// A dismissed artist or track, the latest first.
#[derive(Serialize)]
struct Hidden {
    /// `artist` or `recording`.
    kind: String,
    id: i64,
    name: String,
    /// The artist of a track.
    #[serde(skip_serializing_if = "Option::is_none")]
    artist: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    hidden_at: OffsetDateTime,
}

async fn hidden(
    State(state): State<AppState>,
    Account(account): Account,
    Path(slug): Path<String>,
) -> Result<Json<Vec<Hidden>>, AppError> {
    let (profile, _) = own_profile(&state.db, account, &slug).await?;
    let hidden = sqlx::query_as!(
        Hidden,
        r#"SELECT d.kind, d.entity_id AS id, coalesce(a.name, r.title) AS "name!",
                  ra.name AS "artist?", d.dismissed_at AS hidden_at
             FROM lost_found_dismissal d
             LEFT JOIN artist a ON d.kind = 'artist' AND a.id = d.entity_id
             LEFT JOIN recording r ON d.kind = 'recording' AND r.id = d.entity_id
             LEFT JOIN artist ra ON ra.id = r.artist_id
            WHERE d.profile_id = $1
            ORDER BY d.dismissed_at DESC, d.kind, d.entity_id"#,
        profile,
    )
    .fetch_all(&state.db)
    .await?;
    Ok(Json(hidden))
}

#[derive(Deserialize)]
struct Dismissal {
    kind: String,
    id: i64,
}

fn check_kind(kind: &str) -> Result<(), AppError> {
    match kind {
        "artist" | "recording" => Ok(()),
        _ => Err(AppError::BadRequest(
            "kind must be artist or recording".into(),
        )),
    }
}

/// Takes an artist or track out of the profile's "Lost & found"; again is fine.
async fn hide(
    State(state): State<AppState>,
    Account(account): Account,
    Path(slug): Path<String>,
    Json(dismissal): Json<Dismissal>,
) -> Result<StatusCode, AppError> {
    check_kind(&dismissal.kind)?;
    let (profile, _) = own_profile(&state.db, account, &slug).await?;
    // Merged entries are dismissed as the one they went into.
    let stored = sqlx::query!(
        "INSERT INTO lost_found_dismissal (profile_id, kind, entity_id)
         SELECT $1::bigint, $2::text, coalesce(merged_into, id)
           FROM artist WHERE $2 = 'artist' AND id = $3::bigint
          UNION ALL
         SELECT $1, $2, coalesce(merged_into, id)
           FROM recording WHERE $2 = 'recording' AND id = $3
         ON CONFLICT DO NOTHING
         RETURNING entity_id",
        profile,
        dismissal.kind,
        dismissal.id,
    )
    .fetch_optional(&state.db)
    .await?;
    if stored.is_none() {
        // Nothing new: either it was dismissed already, or there is no such entry.
        let exists = sqlx::query_scalar!(
            r#"SELECT EXISTS (SELECT FROM artist WHERE $1::text = 'artist' AND id = $2::bigint)
                   OR EXISTS (SELECT FROM recording WHERE $1 = 'recording' AND id = $2)
                   AS "exists!""#,
            dismissal.kind,
            dismissal.id,
        )
        .fetch_one(&state.db)
        .await?;
        if !exists {
            return Err(AppError::NotFound);
        }
    }
    Ok(StatusCode::NO_CONTENT)
}

/// Shows a dismissed artist or track again; one that isn't dismissed is fine.
async fn unhide(
    State(state): State<AppState>,
    Account(account): Account,
    Path((slug, kind, id)): Path<(String, String, i64)>,
) -> Result<StatusCode, AppError> {
    check_kind(&kind)?;
    let (profile, _) = own_profile(&state.db, account, &slug).await?;
    sqlx::query!(
        "DELETE FROM lost_found_dismissal WHERE profile_id = $1 AND kind = $2 AND entity_id = $3",
        profile,
        kind,
        id,
    )
    .execute(&state.db)
    .await?;
    Ok(StatusCode::NO_CONTENT)
}
