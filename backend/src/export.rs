//! "Download your data": everything musicbanana keeps about an account as one
//! JSON file, for the account's owner. The listens are in ListenBrainz's import
//! format, so they can move to another service; secrets (the password hash,
//! scrobble tokens, YourSpotify tokens, login cookies) stay out.
//!
//! A profile can have hundreds of thousands of listens, so the file is written
//! while it is sent, a few hundred listens at a time.

use axum::{
    Router,
    body::Body,
    extract::State,
    http::{HeaderValue, header},
    response::{IntoResponse, Response},
    routing::get,
};
use futures_util::{StreamExt, stream};
use serde_json::{Value, json};
use sqlx::PgPool;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use tokio::sync::mpsc;

use crate::{AppError, AppState, account::Account};

pub fn routes() -> Router<AppState> {
    Router::new().route("/me/export", get(export))
}

type Chunk = Result<Vec<u8>, std::io::Error>;

/// Bytes gathered before they go out.
const CHUNK: usize = 64 * 1024;

fn rfc3339(t: OffsetDateTime) -> String {
    t.format(&Rfc3339).unwrap_or_default()
}

fn rfc3339_opt(t: Option<OffsetDateTime>) -> Value {
    t.map_or(Value::Null, |t| Value::String(rfc3339(t)))
}

/// Everything but the profiles: the account, its follows, tokens, connections
/// and logins.
async fn account_part(db: &PgPool, id: i64) -> Result<serde_json::Map<String, Value>, AppError> {
    let account = sqlx::query!(
        r#"SELECT username::text AS "username!", email::text AS "email!", is_admin, created_at
             FROM account WHERE id = $1"#,
        id
    )
    .fetch_one(db)
    .await?;
    let former = sqlx::query!(
        r#"SELECT username::text AS "username!", renamed_at FROM former_username
            WHERE account_id = $1 ORDER BY renamed_at"#,
        id
    )
    .fetch_all(db)
    .await?;
    let following = sqlx::query!(
        r#"SELECT a.username::text AS "username!", p.slug::text AS "profile!", f.created_at,
                  f.accepted_at
             FROM follow f
             JOIN profile p ON p.id = f.profile_id
             JOIN account a ON a.id = p.account_id
            WHERE f.follower_id = $1
            ORDER BY a.username, p.slug"#,
        id
    )
    .fetch_all(db)
    .await?;
    let followers = sqlx::query!(
        r#"SELECT p.slug::text AS "profile!", a.username::text AS "username!", f.created_at,
                  f.accepted_at
             FROM follow f
             JOIN profile p ON p.id = f.profile_id
             JOIN account a ON a.id = f.follower_id
            WHERE p.account_id = $1
            ORDER BY p.slug, a.username"#,
        id
    )
    .fetch_all(db)
    .await?;
    let tokens = sqlx::query!(
        r#"SELECT p.slug::text AS "profile!", t.label, t.created_at, t.last_used_at, t.revoked_at
             FROM api_token t JOIN profile p ON p.id = t.profile_id
            WHERE p.account_id = $1
            ORDER BY t.created_at"#,
        id
    )
    .fetch_all(db)
    .await?;
    let connections = sqlx::query!(
        r#"SELECT p.slug::text AS "profile!", c.api_url, c.created_at, c.finished_at, c.imported
             FROM yourspotify_connection c JOIN profile p ON p.id = c.profile_id
            WHERE p.account_id = $1
            ORDER BY p.slug"#,
        id
    )
    .fetch_all(db)
    .await?;
    let logins = sqlx::query!(
        "SELECT created_at, expires_at FROM session
          WHERE account_id = $1 AND expires_at > now() ORDER BY created_at",
        id
    )
    .fetch_all(db)
    .await?;

    let mut part = serde_json::Map::new();
    part.insert(
        "about".into(),
        json!(
            "Everything musicbanana keeps about this account, except secrets: the password, \
             scrobble tokens, YourSpotify tokens and login cookies. Each profile's listens \
             are in ListenBrainz's import format; listens in its trash come after them."
        ),
    );
    part.insert(
        "exported_at".into(),
        json!(rfc3339(OffsetDateTime::now_utc())),
    );
    part.insert(
        "account".into(),
        json!({
            "username": account.username,
            "email": account.email,
            "admin": account.is_admin,
            "created_at": rfc3339(account.created_at),
            "former_usernames": former.iter().map(|f| json!({
                "username": f.username,
                "renamed_at": rfc3339(f.renamed_at),
            })).collect::<Vec<_>>(),
        }),
    );
    part.insert(
        "following".into(),
        following
            .iter()
            .map(|f| {
                json!({
                    "username": f.username,
                    "profile": f.profile,
                    "asked_at": rfc3339(f.created_at),
                    "accepted_at": rfc3339_opt(f.accepted_at),
                })
            })
            .collect(),
    );
    part.insert(
        "followers".into(),
        followers
            .iter()
            .map(|f| {
                json!({
                    "profile": f.profile,
                    "username": f.username,
                    "asked_at": rfc3339(f.created_at),
                    "accepted_at": rfc3339_opt(f.accepted_at),
                })
            })
            .collect(),
    );
    part.insert(
        "scrobble_tokens".into(),
        tokens
            .iter()
            .map(|t| {
                json!({
                    "profile": t.profile,
                    "label": t.label,
                    "created_at": rfc3339(t.created_at),
                    "last_used_at": rfc3339_opt(t.last_used_at),
                    "revoked_at": rfc3339_opt(t.revoked_at),
                })
            })
            .collect(),
    );
    part.insert(
        "yourspotify_connections".into(),
        connections
            .iter()
            .map(|c| {
                json!({
                    "profile": c.profile,
                    "api_url": c.api_url,
                    "created_at": rfc3339(c.created_at),
                    "last_import_at": rfc3339_opt(c.finished_at),
                    "imported": c.imported,
                })
            })
            .collect(),
    );
    part.insert(
        "logins".into(),
        logins
            .iter()
            .map(|l| {
                json!({
                    "created_at": rfc3339(l.created_at),
                    "expires_at": rfc3339(l.expires_at),
                })
            })
            .collect(),
    );
    Ok(part)
}

/// A listen as ListenBrainz imports it, with what musicbanana made of it.
#[allow(clippy::too_many_arguments)]
fn listen_json(
    listened_at: OffsetDateTime,
    artist: String,
    track: String,
    album: Option<String>,
    album_artist: Option<String>,
    track_number: Option<i16>,
    duration_ms: Option<i32>,
    client: Option<String>,
    extra: Option<Value>,
    submitted_at: OffsetDateTime,
    catalog: [Option<String>; 3],
) -> Value {
    let mut info = match extra {
        Some(Value::Object(map)) => map,
        Some(other) => serde_json::Map::from_iter([("extra".to_owned(), other)]),
        None => serde_json::Map::new(),
    };
    if let Some(n) = track_number {
        info.insert("tracknumber".into(), json!(n));
    }
    if let Some(ms) = duration_ms {
        info.insert("duration_ms".into(), json!(ms));
    }
    if let Some(client) = client {
        info.insert("submission_client".into(), json!(client));
    }
    if let Some(album_artist) = album_artist {
        info.insert("album_artist_name".into(), json!(album_artist));
    }
    let [catalog_artist, catalog_track, catalog_album] = catalog;
    json!({
        "listened_at": listened_at.unix_timestamp(),
        "track_metadata": {
            "artist_name": artist,
            "track_name": track,
            "release_name": album,
            "additional_info": info,
        },
        "musicbanana": {
            "submitted_at": rfc3339(submitted_at),
            "artist": catalog_artist,
            "track": catalog_track,
            "album": catalog_album,
        },
    })
}

/// Hands the file over in chunks. Returns false when the download was dropped.
struct Writer {
    buf: Vec<u8>,
    tx: mpsc::Sender<Chunk>,
}

impl Writer {
    async fn put(&mut self, bytes: &[u8]) -> bool {
        self.buf.extend_from_slice(bytes);
        if self.buf.len() < CHUNK {
            return true;
        }
        self.flush().await
    }

    async fn flush(&mut self) -> bool {
        let chunk = std::mem::take(&mut self.buf);
        self.tx.send(Ok(chunk)).await.is_ok()
    }
}

/// Writes the profiles with their listens and trash after the rest.
async fn write(
    db: PgPool,
    head: serde_json::Map<String, Value>,
    profiles: Vec<(i64, Value)>,
    tx: mpsc::Sender<Chunk>,
) -> anyhow::Result<()> {
    let mut out = Writer {
        buf: Vec::with_capacity(CHUNK * 2),
        tx,
    };
    // The account part without its closing brace, then the profiles.
    let mut start = serde_json::to_vec(&Value::Object(head))?;
    start.pop();
    start.extend_from_slice(b",\"profiles\":[");
    if !out.put(&start).await {
        return Ok(());
    }
    for (n, (id, profile)) in profiles.into_iter().enumerate() {
        let mut opening = serde_json::to_vec(&profile)?;
        opening.pop();
        let prefix: &[u8] = if n == 0 { b"" } else { b"," };
        if !out.put(prefix).await || !out.put(&opening).await || !out.put(b",\"listens\":[").await {
            return Ok(());
        }
        let mut rows = sqlx::query!(
            r#"SELECT l.listened_at, l.artist_raw, l.track_raw, l.album_raw, l.album_artist_raw,
                      l.track_number, l.duration_ms, l.client, l.extra, l.submitted_at,
                      a.name AS "catalog_artist?", r.title AS "catalog_track?",
                      rel.title AS "catalog_album?"
                 FROM listen l
                 JOIN artist a ON a.id = l.artist_id
                 JOIN recording r ON r.id = l.recording_id
                 LEFT JOIN release rel ON rel.id = l.release_id
                WHERE l.profile_id = $1
                ORDER BY l.listened_at"#,
            id
        )
        .fetch(&db);
        let mut first = true;
        while let Some(row) = rows.next().await {
            let l = row?;
            let listen = listen_json(
                l.listened_at,
                l.artist_raw,
                l.track_raw,
                l.album_raw,
                l.album_artist_raw,
                l.track_number,
                l.duration_ms,
                l.client,
                l.extra,
                l.submitted_at,
                [l.catalog_artist, l.catalog_track, l.catalog_album],
            );
            let mut bytes = if first { Vec::new() } else { vec![b','] };
            first = false;
            serde_json::to_writer(&mut bytes, &listen)?;
            if !out.put(&bytes).await {
                return Ok(());
            }
        }
        drop(rows);
        if !out.put(b"],\"trash\":[").await {
            return Ok(());
        }
        let trash = sqlx::query!(
            r#"SELECT t.listened_at, t.artist_raw, t.track_raw, t.album_raw, t.album_artist_raw,
                      t.track_number, t.duration_ms, t.client, t.extra, t.submitted_at,
                      t.trashed_at, a.name AS "catalog_artist?", r.title AS "catalog_track?",
                      rel.title AS "catalog_album?"
                 FROM listen_trash t
                 JOIN artist a ON a.id = t.artist_id
                 JOIN recording r ON r.id = t.recording_id
                 LEFT JOIN release rel ON rel.id = t.release_id
                WHERE t.profile_id = $1
                ORDER BY t.listened_at"#,
            id
        )
        .fetch_all(&db)
        .await?;
        for (i, t) in trash.into_iter().enumerate() {
            let mut listen = listen_json(
                t.listened_at,
                t.artist_raw,
                t.track_raw,
                t.album_raw,
                t.album_artist_raw,
                t.track_number,
                t.duration_ms,
                t.client,
                t.extra,
                t.submitted_at,
                [t.catalog_artist, t.catalog_track, t.catalog_album],
            );
            listen["musicbanana"]["trashed_at"] = json!(rfc3339(t.trashed_at));
            let mut bytes = if i == 0 { Vec::new() } else { vec![b','] };
            serde_json::to_writer(&mut bytes, &listen)?;
            if !out.put(&bytes).await {
                return Ok(());
            }
        }
        if !out.put(b"]}").await {
            return Ok(());
        }
    }
    if out.put(b"]}\n").await {
        out.flush().await;
    }
    Ok(())
}

async fn export(State(state): State<AppState>, Account(id): Account) -> Result<Response, AppError> {
    let db = state.db.clone();
    let head = account_part(&db, id).await?;
    let username = head["account"]["username"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    let profiles = sqlx::query!(
        r#"SELECT p.id, p.slug::text AS "slug!", p.name, p.visibility::text AS "visibility!",
                  p.created_at, (SELECT count(*) FROM listen l WHERE l.profile_id = p.id) AS "listens!"
             FROM profile p WHERE p.account_id = $1 ORDER BY p.slug"#,
        id
    )
    .fetch_all(&db)
    .await?
    .into_iter()
    .map(|p| {
        let profile = json!({
            "slug": p.slug,
            "name": p.name,
            "visibility": p.visibility,
            "created_at": rfc3339(p.created_at),
            "listen_count": p.listens,
        });
        (p.id, profile)
    })
    .collect();
    tracing::info!("{username} downloads their data");

    let (tx, rx) = mpsc::channel::<Chunk>(4);
    tokio::spawn(async move {
        let failed = tx.clone();
        if let Err(e) = write(db, head, profiles, tx).await {
            tracing::warn!("data download: {e:#}");
            // Cuts the download off, so it doesn't look complete.
            let _ = failed.send(Err(std::io::Error::other(e.to_string()))).await;
        }
    });
    let body = Body::from_stream(stream::unfold(rx, |mut rx| async move {
        rx.recv().await.map(|chunk| (chunk, rx))
    }));

    let day = OffsetDateTime::now_utc().date();
    let file = format!("musicbanana-{username}-{day}.json");
    let mut response = body.into_response();
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    // User names are letters, digits, dashes, dots and underscores; others get a plain name.
    let disposition = HeaderValue::from_str(&format!("attachment; filename=\"{file}\""))
        .unwrap_or_else(|_| HeaderValue::from_static("attachment; filename=\"musicbanana.json\""));
    headers.insert(header::CONTENT_DISPOSITION, disposition);
    Ok(response)
}
