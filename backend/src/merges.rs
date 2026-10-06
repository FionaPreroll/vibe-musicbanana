//! The page "Merges" for admins: the suggestions of `merge suggest`, merging
//! two entries after a dry run, and the merge log with undo. It does what the
//! `merge` commands do, with the same code in [`crate::merge`].

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{delete, get, post},
};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::{
    AppError, AppState,
    account::Admin,
    merge::{self, Kind, Likeness},
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/admin/merges", get(log).post(do_merge))
        .route("/admin/merges/{op}/undo", post(undo))
        .route("/admin/merges/suggestions/{kind}", get(suggestions))
        .route("/admin/merges/hidden/{kind}", get(hidden).post(hide))
        .route("/admin/merges/hidden/{kind}/{from}/{into}", delete(unhide))
        .route("/admin/catalog/{kind}", get(find))
}

fn kind(name: &str) -> Result<Kind, AppError> {
    match name {
        "artist" => Ok(Kind::Artist),
        "release" => Ok(Kind::Release),
        "recording" => Ok(Kind::Recording),
        _ => Err(AppError::NotFound),
    }
}

/// The merge code's refusals ("was merged into 7 already") as 409 with their
/// text; database trouble stays a 500.
fn refused(err: anyhow::Error) -> AppError {
    if err.downcast_ref::<sqlx::Error>().is_some() {
        AppError::Internal(err)
    } else {
        AppError::Status(StatusCode::CONFLICT, format!("{err:#}"))
    }
}

#[derive(Serialize)]
struct Entry {
    id: i64,
    name: String,
    /// The artist of a release or recording.
    artist: Option<String>,
    listens: i64,
}

impl From<merge::Entry> for Entry {
    fn from(e: merge::Entry) -> Self {
        Entry {
            id: e.id,
            name: e.name,
            artist: e.artist,
            listens: e.listens,
        }
    }
}

// ---------------------------------------------------------------- suggestions

#[derive(Deserialize)]
struct Limit {
    /// 100 by default, at most 500.
    limit: Option<usize>,
}

#[derive(Serialize)]
struct Suggestion {
    from: Entry,
    into: Entry,
    likeness: &'static str,
}

#[derive(Serialize)]
struct Suggestions {
    /// All of them, of which `suggestions` are the first.
    total: usize,
    suggestions: Vec<Suggestion>,
    /// How many the admins hid.
    hidden: i64,
}

async fn suggestions(
    State(state): State<AppState>,
    _: Admin,
    Path(name): Path<String>,
    Query(params): Query<Limit>,
) -> Result<Json<Suggestions>, AppError> {
    let kind = kind(&name)?;
    let found = merge::suggest(&state.db, kind).await?;
    let hidden = sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!" FROM merge_dismissal WHERE kind = $1"#,
        kind.to_string()
    )
    .fetch_one(&state.db)
    .await?;
    let total = found.len();
    let suggestions = found
        .into_iter()
        .take(params.limit.unwrap_or(100).clamp(1, 500))
        .map(|s| Suggestion {
            from: s.from.into(),
            into: s.into.into(),
            likeness: match s.likeness {
                Likeness::SameLetters => "same_letters",
                Likeness::OneLetter => "one_letter",
                Likeness::Version => "version",
            },
        })
        .collect();
    Ok(Json(Suggestions {
        total,
        suggestions,
        hidden,
    }))
}

#[derive(Deserialize)]
struct Pair {
    from: i64,
    into: i64,
}

#[derive(Serialize)]
struct HiddenPair {
    from: (i64, String),
    into: (i64, String),
    #[serde(with = "time::serde::rfc3339")]
    hidden_at: OffsetDateTime,
}

async fn hidden(
    State(state): State<AppState>,
    _: Admin,
    Path(name): Path<String>,
) -> Result<Json<Vec<HiddenPair>>, AppError> {
    let hidden = merge::hidden(&state.db, kind(&name)?).await?;
    Ok(Json(
        hidden
            .into_iter()
            .map(|h| HiddenPair {
                from: h.from,
                into: h.into,
                hidden_at: h.hidden_at,
            })
            .collect(),
    ))
}

async fn hide(
    State(state): State<AppState>,
    Admin(admin): Admin,
    Path(name): Path<String>,
    Json(pair): Json<Pair>,
) -> Result<StatusCode, AppError> {
    let kind = kind(&name)?;
    merge::hide(&state.db, kind, pair.from, pair.into)
        .await
        .map_err(refused)?;
    tracing::info!(
        target: "musicbanana::merge",
        "account {admin} hid the suggestion {kind} {} into {}",
        pair.from,
        pair.into
    );
    Ok(StatusCode::NO_CONTENT)
}

async fn unhide(
    State(state): State<AppState>,
    _: Admin,
    Path((name, from, into)): Path<(String, i64, i64)>,
) -> Result<StatusCode, AppError> {
    if merge::unhide(&state.db, kind(&name)?, from, into).await? {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound)
    }
}

// ---------------------------------------------------------------- finding entries

#[derive(Deserialize)]
struct Find {
    /// Words that must all be in the name (or the artist's name), or an id.
    q: String,
}

/// Entries that have not been merged, to merge one into another by hand, the
/// most heard first. A number finds the entry with that id.
async fn find(
    State(state): State<AppState>,
    _: Admin,
    Path(name): Path<String>,
    Query(params): Query<Find>,
) -> Result<Json<Vec<Entry>>, AppError> {
    let kind = kind(&name)?;
    let words: Vec<&str> = params.q.split_whitespace().take(10).collect();
    if words.is_empty() {
        return Ok(Json(vec![]));
    }
    let id = params.q.trim().trim_start_matches('#').parse::<i64>().ok();
    let db = &state.db;
    let found = match kind {
        Kind::Artist => sqlx::query!(
            r#"WITH w AS (SELECT search_fold(x) AS w FROM unnest($1::text[]) x)
               SELECT a.id, a.name, NULL::text AS artist,
                      (SELECT count(*) FROM listen l WHERE l.artist_id = a.id) AS "listens!"
                 FROM artist a
                WHERE a.merged_into IS NULL
                  AND (a.id = $2 OR NOT EXISTS (SELECT FROM w WHERE strpos(a.search_name, w) = 0))
                ORDER BY a.id = $2 DESC, 4 DESC, a.name, a.id
                LIMIT 20"#,
            &words as &[&str],
            id
        )
        .fetch_all(db)
        .await?
        .into_iter()
        .map(|r| Entry {
            id: r.id,
            name: r.name,
            artist: r.artist,
            listens: r.listens,
        })
        .collect(),
        Kind::Release => sqlx::query!(
            r#"WITH w AS (SELECT search_fold(x) AS w FROM unnest($1::text[]) x)
               SELECT r.id, r.title, a.name AS artist,
                      (SELECT count(*) FROM listen l WHERE l.release_id = r.id) AS "listens!"
                 FROM release r
                 JOIN artist a ON a.id = r.artist_id
                WHERE r.merged_into IS NULL
                  AND (r.id = $2 OR NOT EXISTS (SELECT FROM w WHERE strpos(r.search_name, w) = 0
                                                                AND strpos(a.search_name, w) = 0))
                ORDER BY r.id = $2 DESC, 4 DESC, r.title, r.id
                LIMIT 20"#,
            &words as &[&str],
            id
        )
        .fetch_all(db)
        .await?
        .into_iter()
        .map(|r| Entry {
            id: r.id,
            name: r.title,
            artist: Some(r.artist),
            listens: r.listens,
        })
        .collect(),
        Kind::Recording => sqlx::query!(
            r#"WITH w AS (SELECT search_fold(x) AS w FROM unnest($1::text[]) x)
               SELECT r.id, r.title, a.name AS artist,
                      (SELECT count(*) FROM listen l WHERE l.recording_id = r.id) AS "listens!"
                 FROM recording r
                 JOIN artist a ON a.id = r.artist_id
                WHERE r.merged_into IS NULL
                  AND (r.id = $2 OR NOT EXISTS (SELECT FROM w WHERE strpos(r.search_name, w) = 0
                                                                AND strpos(a.search_name, w) = 0))
                ORDER BY r.id = $2 DESC, 4 DESC, r.title, r.id
                LIMIT 20"#,
            &words as &[&str],
            id
        )
        .fetch_all(db)
        .await?
        .into_iter()
        .map(|r| Entry {
            id: r.id,
            name: r.title,
            artist: Some(r.artist),
            listens: r.listens,
        })
        .collect(),
    };
    Ok(Json(found))
}

// ---------------------------------------------------------------- merging

#[derive(Deserialize)]
struct MergeRequest {
    kind: String,
    from: i64,
    into: i64,
    /// Only report what the merge would change.
    #[serde(default)]
    dry_run: bool,
    /// Merge even though the MusicBrainz IDs tell the two apart.
    #[serde(default)]
    force: bool,
}

#[derive(Serialize)]
struct Merged {
    kind: String,
    from: (i64, String),
    into: (i64, String),
    dry_run: bool,
    listens: u64,
    spellings: u64,
    releases_moved: u64,
    releases_merged: u64,
    recordings_moved: u64,
    recordings_merged: u64,
    /// Whether the MusicBrainz IDs tell the two apart, so that merging needs `force`.
    told_apart: bool,
    /// The merge's number in the log; none in a dry run.
    op: Option<i64>,
}

/// A dry run always goes through, with `told_apart` saying whether the real
/// merge needs `force`.
async fn do_merge(
    State(state): State<AppState>,
    Admin(admin): Admin,
    Json(request): Json<MergeRequest>,
) -> Result<Json<Merged>, AppError> {
    let kind = kind(&request.kind).map_err(|_| AppError::BadRequest("unknown kind".into()))?;
    let options = merge::Options {
        dry_run: request.dry_run,
        force: request.force || request.dry_run,
    };
    let m = merge::merge(&state.db, kind, request.from, request.into, options)
        .await
        .map_err(refused)?;
    if let Some(op) = m.op {
        tracing::info!(
            target: "musicbanana::merge",
            "account {admin} merged {kind} {} into {} (merge {op}, {} listens)",
            m.from.0,
            m.into.0,
            m.listens
        );
    }
    Ok(Json(Merged {
        kind: kind.to_string(),
        from: m.from,
        into: m.into,
        dry_run: m.dry_run,
        listens: m.listens,
        spellings: m.spellings,
        releases_moved: m.releases_moved,
        releases_merged: m.releases_merged,
        recordings_moved: m.recordings_moved,
        recordings_merged: m.recordings_merged,
        told_apart: m.told_apart,
        op: m.op,
    }))
}

// ---------------------------------------------------------------- the log

#[derive(Serialize)]
struct MergeOp {
    id: i64,
    kind: String,
    from: (i64, String),
    into: (i64, String),
    #[serde(with = "time::serde::rfc3339")]
    merged_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    undone_at: Option<OffsetDateTime>,
}

impl From<merge::MergeOp> for MergeOp {
    fn from(op: merge::MergeOp) -> Self {
        MergeOp {
            id: op.id,
            kind: op.kind,
            from: op.from,
            into: op.into,
            merged_at: op.merged_at,
            undone_at: op.undone_at,
        }
    }
}

#[derive(Deserialize)]
struct LogParams {
    /// Only merges before this one, for the next page.
    before: Option<i64>,
    /// 50 by default, at most 200.
    limit: Option<i64>,
}

#[derive(Serialize)]
struct Log {
    merges: Vec<MergeOp>,
    /// Pass as `before` for the following, older page.
    next: Option<i64>,
}

async fn log(
    State(state): State<AppState>,
    _: Admin,
    Query(params): Query<LogParams>,
) -> Result<Json<Log>, AppError> {
    let limit = params.limit.unwrap_or(50).clamp(1, 200);
    // One more than asked, to know whether there is a next page.
    let mut ops = merge::log_before(&state.db, params.before, limit + 1).await?;
    let next = (ops.len() as i64 > limit).then(|| {
        ops.truncate(limit as usize);
        ops.last().map(|op| op.id)
    });
    Ok(Json(Log {
        merges: ops.into_iter().map(Into::into).collect(),
        next: next.flatten(),
    }))
}

#[derive(Deserialize, Default)]
struct UndoRequest {
    #[serde(default)]
    dry_run: bool,
}

#[derive(Serialize)]
struct Undone {
    merge: MergeOp,
    restored: i64,
    kept: i64,
    dry_run: bool,
}

async fn undo(
    State(state): State<AppState>,
    Admin(admin): Admin,
    Path(op): Path<i64>,
    Json(request): Json<UndoRequest>,
) -> Result<Json<Undone>, AppError> {
    let exists = sqlx::query_scalar!("SELECT id FROM merge_op WHERE id = $1", op)
        .fetch_optional(&state.db)
        .await?;
    if exists.is_none() {
        return Err(AppError::NotFound);
    }
    let done = merge::undo(&state.db, op, request.dry_run)
        .await
        .map_err(refused)?;
    if !done.dry_run {
        tracing::info!(target: "musicbanana::merge", "account {admin} undid merge {op}");
    }
    Ok(Json(Undone {
        merge: done.op.into(),
        restored: done.restored,
        kept: done.kept,
        dry_run: done.dry_run,
    }))
}
