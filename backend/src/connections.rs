//! YourSpotify accounts connected to profiles, which the server keeps importing
//! from: a new connection's whole history right away, then the new plays every
//! 15 minutes (see [`crate::yourspotify`]).

use anyhow::Context;
use serde::Serialize;
use sqlx::PgPool;
use time::OffsetDateTime;

use crate::yourspotify::{self, Source};

/// How often the new plays are fetched.
pub const EVERY: std::time::Duration = std::time::Duration::from_secs(15 * 60);

/// How often the server looks for connections that are due.
const TICK: std::time::Duration = std::time::Duration::from_secs(60);

#[derive(Debug, Serialize)]
pub struct Connection {
    pub id: i64,
    pub username: String,
    /// The profile's slug.
    pub profile: String,
    pub url: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    pub started_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub finished_at: Option<OffsetDateTime>,
    /// Listens it brought so far.
    pub imported: i64,
    /// Why the latest import failed.
    pub error: Option<String>,
}

/// Connects a YourSpotify account to a profile, once YourSpotify has taken the
/// token. The server imports from it in its next round, within a minute.
pub async fn add(db: &PgPool, profile_id: i64, url: &str, token: &str) -> anyhow::Result<i64> {
    let taken = sqlx::query_scalar!(
        "SELECT id FROM yourspotify_connection WHERE profile_id = $1",
        profile_id
    )
    .fetch_optional(db)
    .await?;
    if let Some(id) = taken {
        anyhow::bail!(
            "the profile is connected to YourSpotify already (connection {id}); \
             remove that one first"
        );
    }
    Source::new(url, token)?.check().await?;
    let id = sqlx::query_scalar!(
        "INSERT INTO yourspotify_connection (profile_id, api_url, token)
         VALUES ($1, $2, $3) RETURNING id",
        profile_id,
        url.trim().trim_end_matches('/'),
        token.trim(),
    )
    .fetch_one(db)
    .await
    .context("the profile is connected to YourSpotify already")?;
    Ok(id)
}

/// The connections of an account, or of all accounts.
pub async fn list(db: &PgPool, account: Option<i64>) -> sqlx::Result<Vec<Connection>> {
    sqlx::query_as!(
        Connection,
        r#"SELECT c.id, a.username::text AS "username!", p.slug::text AS "profile!",
                  c.api_url AS url, c.created_at, c.started_at, c.finished_at, c.imported, c.error
             FROM yourspotify_connection c
             JOIN profile p ON p.id = c.profile_id
             JOIN account a ON a.id = p.account_id
            WHERE $1::bigint IS NULL OR a.id = $1
            ORDER BY a.username, p.slug"#,
        account,
    )
    .fetch_all(db)
    .await
}

/// Removes a connection (of the account, if given); the listens it brought stay.
pub async fn remove(db: &PgPool, id: i64, account: Option<i64>) -> sqlx::Result<bool> {
    let removed = sqlx::query!(
        "DELETE FROM yourspotify_connection c
          USING profile p
          WHERE p.id = c.profile_id AND c.id = $1 AND ($2::bigint IS NULL OR p.account_id = $2)",
        id,
        account,
    )
    .execute(db)
    .await?;
    Ok(removed.rows_affected() > 0)
}

/// Imports from every connection that is due: new ones, and the others
/// [`EVERY`] after their latest import. Returns how many it imported from.
///
/// Taking a connection sets its start, which keeps it from being due again
/// while the import runs, so no two imports of one overlap, also not across
/// servers on one database.
pub async fn sync_due(db: &PgPool) -> anyhow::Result<usize> {
    let due = sqlx::query!(
        r#"UPDATE yourspotify_connection c
              SET started_at = now()
             FROM profile p
             JOIN account a ON a.id = p.account_id
            WHERE p.id = c.profile_id
              AND (c.started_at IS NULL
                   OR (c.finished_at >= c.started_at
                       AND c.finished_at < now() - make_interval(secs => $1))
                   -- An import that died with its server.
                   OR (c.finished_at IS NULL OR c.finished_at < c.started_at)
                      AND c.started_at < now() - interval '1 day')
           RETURNING c.id, c.profile_id, c.api_url, c.token, c.finished_at,
                     a.username::text AS "username!", p.slug::text AS "slug!""#,
        EVERY.as_secs_f64(),
    )
    .fetch_all(db)
    .await?;
    // Each on its own, so that the first import of a long history holds up no
    // other connection.
    let mut imports = tokio::task::JoinSet::new();
    for c in due {
        let db = db.clone();
        let what = format!(
            "YourSpotify connection {} ({}/{})",
            c.id, c.username, c.slug
        );
        tracing::info!(
            "{what}: fetching {} from {}",
            if c.finished_at.is_none() {
                "the history"
            } else {
                "new plays"
            },
            c.api_url
        );
        imports
            .spawn(async move { sync(&db, c.id, &what, c.profile_id, &c.api_url, &c.token).await });
    }
    let mut count = 0;
    while let Some(done) = imports.join_next().await {
        done??;
        count += 1;
    }
    Ok(count)
}

async fn sync(
    db: &PgPool,
    id: i64,
    what: &str,
    profile_id: i64,
    url: &str,
    token: &str,
) -> sqlx::Result<()> {
    let result = match Source::new(url, token) {
        Ok(source) => yourspotify::import(db, &source, profile_id, false).await,
        Err(e) => Err(e),
    };
    let (imported, error) = match &result {
        Ok(report) => {
            tracing::info!("{what}: {report}");
            (report.recorded as i64, None)
        }
        Err(e) => {
            tracing::warn!("{what}: {e:#}");
            (0, Some(format!("{e:#}")))
        }
    };
    sqlx::query!(
        "UPDATE yourspotify_connection
            SET finished_at = now(), imported = imported + $2, error = $3
          WHERE id = $1",
        id,
        imported,
        error,
    )
    .execute(db)
    .await?;
    Ok(())
}

static LAST_ROUND: std::sync::Mutex<Option<OffsetDateTime>> = std::sync::Mutex::new(None);

/// When the server last looked for connections that are due.
pub fn last_round() -> Option<OffsetDateTime> {
    *LAST_ROUND.lock().unwrap_or_else(|e| e.into_inner())
}

/// Keeps importing from the connections while the server runs. A round does not
/// wait for the imports of the one before; those are skipped until they are done.
pub fn spawn(db: PgPool) {
    tokio::spawn(async move {
        loop {
            let db = db.clone();
            *LAST_ROUND.lock().unwrap_or_else(|e| e.into_inner()) = Some(OffsetDateTime::now_utc());
            tokio::spawn(async move {
                if let Err(e) = sync_due(&db).await {
                    tracing::warn!("YourSpotify connections: {e:#}");
                }
            });
            tokio::time::sleep(TICK).await;
        }
    });
}
