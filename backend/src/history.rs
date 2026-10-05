//! "Edit listening history": the owner of a profile looks through its listens,
//! moves wrong ones to the trash, restores them from there or empties it (see
//! migrations/0012_listen_trash.sql). Only the owner gets here; the trash is
//! the undo.

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use time::OffsetDateTime;

use crate::{AppError, AppState, account::Account};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/me/profiles/{slug}/history", get(history))
        .route("/me/profiles/{slug}/trash", get(trash).post(move_to_trash))
        .route("/me/profiles/{slug}/trash/restore", post(restore))
        .route("/me/profiles/{slug}/trash/empty", post(empty))
}

/// Which listens: every word in the artist, track or album name (ignoring case
/// and accents), from and to a time, from one source.
#[derive(Deserialize, Default)]
struct Filter {
    q: Option<String>,
    #[serde(default, with = "time::serde::rfc3339::option")]
    from: Option<OffsetDateTime>,
    #[serde(default, with = "time::serde::rfc3339::option")]
    to: Option<OffsetDateTime>,
    source: Option<String>,
}

impl Filter {
    fn words(&self) -> Vec<String> {
        self.q
            .as_deref()
            .unwrap_or_default()
            .split_whitespace()
            .map(str::to_owned)
            .collect()
    }

    fn is_empty(&self) -> bool {
        self.words().is_empty() && self.from.is_none() && self.to.is_none() && self.source.is_none()
    }
}

#[derive(Deserialize)]
struct PageParams {
    #[serde(flatten)]
    filter: Filter,
    /// Only listens strictly before this time; a page's `next`.
    #[serde(default, with = "time::serde::rfc3339::option")]
    before: Option<OffsetDateTime>,
    limit: Option<i64>,
}

#[derive(Serialize)]
struct Entry {
    id: i64,
    #[serde(with = "time::serde::rfc3339")]
    listened_at: OffsetDateTime,
    artist: String,
    track: String,
    album: Option<String>,
    source: String,
    #[serde(with = "time::serde::rfc3339::option")]
    trashed_at: Option<OffsetDateTime>,
}

#[derive(Serialize)]
struct Page {
    listens: Vec<Entry>,
    /// Set when there are older listens.
    #[serde(with = "time::serde::rfc3339::option")]
    next: Option<OffsetDateTime>,
    /// How many listens there are in all (that match).
    total: i64,
}

/// The id of the viewer's profile `slug`.
async fn own_profile(db: &PgPool, account: i64, slug: &str) -> Result<(i64, String), AppError> {
    let profile = sqlx::query!(
        r#"SELECT p.id, a.username::text || '/' || p.slug::text AS "name!"
             FROM profile p JOIN account a ON a.id = p.account_id
            WHERE p.account_id = $1 AND p.slug = $2::text::citext"#,
        account,
        slug,
    )
    .fetch_optional(db)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok((profile.id, profile.name))
}

fn paged(mut listens: Vec<Entry>, limit: i64, total: i64) -> Page {
    let next = if listens.len() as i64 > limit {
        listens.truncate(limit as usize);
        listens.last().map(|l| l.listened_at)
    } else {
        None
    };
    Page {
        listens,
        next,
        total,
    }
}

async fn history(
    State(state): State<AppState>,
    Account(id): Account,
    Path(slug): Path<String>,
    Query(params): Query<PageParams>,
) -> Result<Json<Page>, AppError> {
    let (profile, _) = own_profile(&state.db, id, &slug).await?;
    let limit = params.limit.unwrap_or(100).clamp(1, 500);
    let f = &params.filter;
    let words = f.words();
    // One row more than asked for tells whether there is another page; the
    // count of all that match comes along in every row.
    let rows = sqlx::query!(
        r#"WITH w AS (SELECT search_fold(x) AS w FROM unnest($6::text[]) x),
                m AS (
             SELECT l.id, l.listened_at, a.name AS artist, r.title AS track,
                    rel.title AS album, listen_source(l.client) AS source
               FROM listen l
               JOIN artist a ON a.id = l.artist_id
               JOIN recording r ON r.id = l.recording_id
               LEFT JOIN release rel ON rel.id = l.release_id
              WHERE l.profile_id = $1
                AND l.listened_at >= coalesce($2::timestamptz, '-infinity')
                AND l.listened_at <= coalesce($3::timestamptz, 'infinity')
                AND ($4::text IS NULL OR listen_source(l.client) = $4)
                AND NOT EXISTS (
                      SELECT FROM w
                       WHERE strpos(a.search_name || ' ' || r.search_name || ' '
                                    || coalesce(rel.search_name, ''), w.w) = 0))
           SELECT m.id AS "id!", m.listened_at AS "listened_at!", m.artist AS "artist!",
                  m.track AS "track!", m.album AS "album?", m.source AS "source!",
                  (SELECT count(*) FROM m) AS "total!"
             FROM m
            WHERE m.listened_at < coalesce($5::timestamptz, 'infinity')
            ORDER BY m.listened_at DESC
            LIMIT $7"#,
        profile,
        f.from,
        f.to,
        f.source,
        params.before,
        &words,
        limit + 1,
    )
    .fetch_all(&state.db)
    .await?;
    let total = match rows.first() {
        Some(row) => row.total,
        None => 0,
    };
    let listens = rows
        .into_iter()
        .map(|r| Entry {
            id: r.id,
            listened_at: r.listened_at,
            artist: r.artist,
            track: r.track,
            album: r.album,
            source: r.source,
            trashed_at: None,
        })
        .collect();
    Ok(Json(paged(listens, limit, total)))
}

/// Some listens by their ids, or all that match a filter.
#[derive(Deserialize)]
struct Selection {
    ids: Option<Vec<i64>>,
    #[serde(flatten)]
    filter: Filter,
}

#[derive(Serialize)]
struct Moved {
    /// The listens now in the trash, for an undo.
    ids: Vec<i64>,
}

async fn move_to_trash(
    State(state): State<AppState>,
    Account(id): Account,
    Path(slug): Path<String>,
    Json(selection): Json<Selection>,
) -> Result<Json<Moved>, AppError> {
    let (profile, name) = own_profile(&state.db, id, &slug).await?;
    let ids = match selection.ids {
        Some(ids) => ids,
        None if selection.filter.is_empty() => {
            return Err(AppError::BadRequest(
                "pick listens, or narrow the history down first".into(),
            ));
        }
        None => {
            let f = &selection.filter;
            sqlx::query_scalar!(
                r#"WITH w AS (SELECT search_fold(x) AS w FROM unnest($5::text[]) x)
                   SELECT l.id
                     FROM listen l
                     JOIN artist a ON a.id = l.artist_id
                     JOIN recording r ON r.id = l.recording_id
                     LEFT JOIN release rel ON rel.id = l.release_id
                    WHERE l.profile_id = $1
                      AND l.listened_at >= coalesce($2::timestamptz, '-infinity')
                      AND l.listened_at <= coalesce($3::timestamptz, 'infinity')
                      AND ($4::text IS NULL OR listen_source(l.client) = $4)
                      AND NOT EXISTS (
                            SELECT FROM w
                             WHERE strpos(a.search_name || ' ' || r.search_name || ' '
                                          || coalesce(rel.search_name, ''), w.w) = 0)"#,
                profile,
                f.from,
                f.to,
                f.source,
                &f.words(),
            )
            .fetch_all(&state.db)
            .await?
        }
    };
    let moved = sqlx::query_scalar!(
        r#"WITH gone AS (DELETE FROM listen WHERE profile_id = $1 AND id = ANY($2) RETURNING *)
           INSERT INTO listen_trash (id, profile_id, listened_at, artist_raw, track_raw,
                                     album_raw, album_artist_raw, track_number, duration_ms,
                                     client, extra, artist_id, recording_id, release_id,
                                     submitted_at)
           SELECT id, profile_id, listened_at, artist_raw, track_raw, album_raw,
                  album_artist_raw, track_number, duration_ms, client, extra, artist_id,
                  recording_id, release_id, submitted_at
             FROM gone
           RETURNING id"#,
        profile,
        &ids,
    )
    .fetch_all(&state.db)
    .await?;
    tracing::info!("{name}: moved {} listens to the trash", moved.len());
    Ok(Json(Moved { ids: moved }))
}

async fn trash(
    State(state): State<AppState>,
    Account(id): Account,
    Path(slug): Path<String>,
    Query(params): Query<PageParams>,
) -> Result<Json<Page>, AppError> {
    let (profile, _) = own_profile(&state.db, id, &slug).await?;
    let limit = params.limit.unwrap_or(100).clamp(1, 500);
    let listens = sqlx::query_as!(
        Entry,
        r#"SELECT t.id, t.listened_at, a.name AS artist, r.title AS track,
                  rel.title AS "album?", listen_source(t.client) AS "source!",
                  t.trashed_at AS "trashed_at?"
             FROM listen_trash t
             JOIN artist a ON a.id = t.artist_id
             JOIN recording r ON r.id = t.recording_id
             LEFT JOIN release rel ON rel.id = t.release_id
            WHERE t.profile_id = $1 AND t.listened_at < coalesce($2::timestamptz, 'infinity')
            ORDER BY t.listened_at DESC
            LIMIT $3"#,
        profile,
        params.before,
        limit + 1,
    )
    .fetch_all(&state.db)
    .await?;
    let total = sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!" FROM listen_trash WHERE profile_id = $1"#,
        profile
    )
    .fetch_one(&state.db)
    .await?;
    Ok(Json(paged(listens, limit, total)))
}

/// Listens in the trash by their ids, or all of them.
#[derive(Deserialize)]
struct TrashSelection {
    ids: Option<Vec<i64>>,
}

#[derive(Serialize)]
struct Restored {
    restored: u64,
    /// Left in the trash: the profile has another listen at that time now.
    kept: u64,
}

async fn restore(
    State(state): State<AppState>,
    Account(id): Account,
    Path(slug): Path<String>,
    Json(selection): Json<TrashSelection>,
) -> Result<Json<Restored>, AppError> {
    let (profile, name) = own_profile(&state.db, id, &slug).await?;
    let mut tx = state.db.begin().await?;
    // Back with their ids; catalog entries merged since then count for the
    // ones they were merged into.
    let restored = sqlx::query_scalar!(
        r#"WITH back AS (
             INSERT INTO listen (id, profile_id, listened_at, artist_raw, track_raw, album_raw,
                                 album_artist_raw, track_number, duration_ms, client, extra,
                                 artist_id, recording_id, release_id, submitted_at)
             OVERRIDING SYSTEM VALUE
             SELECT t.id, t.profile_id, t.listened_at, t.artist_raw, t.track_raw, t.album_raw,
                    t.album_artist_raw, t.track_number, t.duration_ms, t.client, t.extra,
                    coalesce(a.merged_into, a.id), coalesce(r.merged_into, r.id),
                    coalesce(rel.merged_into, rel.id), t.submitted_at
               FROM listen_trash t
               JOIN artist a ON a.id = t.artist_id
               JOIN recording r ON r.id = t.recording_id
               LEFT JOIN release rel ON rel.id = t.release_id
              WHERE t.profile_id = $1 AND ($2::int8[] IS NULL OR t.id = ANY($2))
             ON CONFLICT DO NOTHING
             RETURNING id)
           DELETE FROM listen_trash WHERE id IN (SELECT id FROM back) RETURNING id"#,
        profile,
        selection.ids.as_deref(),
    )
    .fetch_all(&mut *tx)
    .await?;
    let kept = sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!" FROM listen_trash
            WHERE profile_id = $1 AND ($2::int8[] IS NULL OR id = ANY($2))
              AND id <> ALL($3)"#,
        profile,
        selection.ids.as_deref(),
        &restored,
    )
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    tracing::info!("{name}: restored {} listens from the trash", restored.len());
    Ok(Json(Restored {
        restored: restored.len() as u64,
        kept: kept as u64,
    }))
}

#[derive(Serialize)]
struct Emptied {
    deleted: u64,
}

/// Deletes listens in the trash for good.
async fn empty(
    State(state): State<AppState>,
    Account(id): Account,
    Path(slug): Path<String>,
    Json(selection): Json<TrashSelection>,
) -> Result<Json<Emptied>, AppError> {
    let (profile, name) = own_profile(&state.db, id, &slug).await?;
    let deleted = sqlx::query!(
        "DELETE FROM listen_trash WHERE profile_id = $1 AND ($2::int8[] IS NULL OR id = ANY($2))",
        profile,
        selection.ids.as_deref(),
    )
    .execute(&state.db)
    .await?
    .rows_affected();
    tracing::info!("{name}: deleted {deleted} listens in the trash for good");
    Ok(Json(Emptied { deleted }))
}
