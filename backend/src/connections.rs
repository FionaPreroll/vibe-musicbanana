//! YourSpotify accounts connected to profiles, which the server keeps importing
//! from: a new connection's whole history right away, then the new plays every
//! 15 minutes (see [`crate::yourspotify`]). On request, it fetches the whole
//! history once more, for plays YourSpotify has added from before the latest
//! one, such as those of an older Spotify export.

use anyhow::Context;
use serde::Serialize;
use sqlx::PgPool;
use time::OffsetDateTime;

use crate::yourspotify::{self, Source, Start};

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
    /// When fetching the whole history again was asked for, while it is not done.
    #[serde(with = "time::serde::rfc3339::option")]
    pub refetch_requested_at: Option<OffsetDateTime>,
    /// How far that has got: the plays before this time are done.
    #[serde(with = "time::serde::rfc3339::option")]
    pub refetch_at: Option<OffsetDateTime>,
    /// The plays it found so far (or the last time), those new to the profile,
    /// and those left out as YourSpotify knows none of their artists; the
    /// others were in the profile already.
    pub refetch_plays: i64,
    pub refetch_new: i64,
    pub refetch_left_out: i64,
    /// When the history was last fetched again in full.
    #[serde(with = "time::serde::rfc3339::option")]
    pub refetched_at: Option<OffsetDateTime>,
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
                  c.api_url AS url, c.created_at, c.started_at, c.finished_at, c.imported, c.error,
                  c.refetch_requested_at, c.refetch_at, c.refetch_plays, c.refetch_new,
                  c.refetch_left_out, c.refetched_at
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

/// Has the server fetch a connection's whole history again (of the account, if
/// given), within a minute, or once the import that runs is done. Plays the
/// profile has are skipped. Asked again before it is done, it changes nothing.
/// Returns whether there is such a connection.
pub async fn refetch(db: &PgPool, id: i64, account: Option<i64>) -> sqlx::Result<bool> {
    let found = sqlx::query_scalar!(
        r#"UPDATE yourspotify_connection c
              SET refetch_requested_at = COALESCE(c.refetch_requested_at, now()),
                  refetch_at = CASE WHEN c.refetch_requested_at IS NULL THEN NULL
                                    ELSE c.refetch_at END,
                  refetch_plays = CASE WHEN c.refetch_requested_at IS NULL THEN 0
                                       ELSE c.refetch_plays END,
                  refetch_new = CASE WHEN c.refetch_requested_at IS NULL THEN 0
                                     ELSE c.refetch_new END,
                  refetch_left_out = CASE WHEN c.refetch_requested_at IS NULL THEN 0
                                          ELSE c.refetch_left_out END
             FROM profile p
            WHERE p.id = c.profile_id AND c.id = $1
              AND ($2::bigint IS NULL OR p.account_id = $2)
        RETURNING c.id"#,
        id,
        account,
    )
    .fetch_optional(db)
    .await?;
    Ok(found.is_some())
}

/// Imports from every connection that is due: new ones, those asked to fetch
/// everything again, and the others [`EVERY`] after their latest import.
/// Returns how many it imported from.
///
/// Taking a connection sets its start, which keeps it from being due again
/// while the import runs, so no two imports of one overlap, also not across
/// servers on one database.
pub async fn sync_due(db: &PgPool) -> anyhow::Result<usize> {
    let due = sqlx::query_as!(
        Taken,
        r#"UPDATE yourspotify_connection c
              SET started_at = now()
             FROM profile p
             JOIN account a ON a.id = p.account_id
            WHERE p.id = c.profile_id
              AND (c.started_at IS NULL
                   OR (c.finished_at >= c.started_at
                       AND (c.finished_at < now() - make_interval(secs => $1)
                            OR c.refetch_requested_at > c.started_at))
                   -- An import that died with its server.
                   OR (c.finished_at IS NULL OR c.finished_at < c.started_at)
                      AND c.started_at < now() - interval '1 day')
           RETURNING c.id, c.profile_id, c.api_url, c.token, c.finished_at, c.imported,
                     c.refetch_requested_at IS NOT NULL AS "refetch!", c.refetch_at,
                     c.refetch_plays, c.refetch_new, c.refetch_left_out,
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
        imports.spawn(async move { c.sync(&db).await });
    }
    let mut count = 0;
    while let Some(done) = imports.join_next().await {
        // A failed import is noted with the connection and logged already.
        let _ = done??;
        count += 1;
    }
    Ok(count)
}

/// A connection the command line can import from, with its token.
pub struct Stored {
    pub id: i64,
    pub username: String,
    /// The profile's slug.
    pub profile: String,
    pub profile_id: i64,
    pub url: String,
    pub token: String,
}

/// The connection to import from on the command line: the one with `id`, or
/// else the only one of the account `username`, in the profile `profile` if
/// given. Fails with a list to pick from when there are several, and says how
/// to connect one when there is none.
pub async fn pick(
    db: &PgPool,
    username: Option<&str>,
    profile: Option<&str>,
    id: Option<i64>,
) -> anyhow::Result<Stored> {
    let mut found = sqlx::query_as!(
        Stored,
        r#"SELECT c.id, a.username::text AS "username!", p.slug::text AS "profile!",
                  p.id AS profile_id, c.api_url AS url, c.token
             FROM yourspotify_connection c
             JOIN profile p ON p.id = c.profile_id
             JOIN account a ON a.id = p.account_id
            WHERE ($1::text IS NULL OR a.username = $1::text::citext)
              AND ($2::text IS NULL OR p.slug = $2::text::citext)
              AND ($3::bigint IS NULL OR c.id = $3)
            ORDER BY p.slug, c.id"#,
        username,
        profile,
        id,
    )
    .fetch_all(db)
    .await?;
    let whose = match (username, profile) {
        (Some(user), Some(profile)) => format!(" of {user}/{profile}"),
        (Some(user), None) => format!(" of {user}"),
        (None, Some(profile)) => format!(" in a profile {profile}"),
        (None, None) => String::new(),
    };
    match (found.len(), id) {
        (1, _) => Ok(found.remove(0)),
        (0, Some(id)) => anyhow::bail!("there is no YourSpotify connection {id}{whose}"),
        (0, None) => anyhow::bail!(
            "there is no YourSpotify connection{whose}: connect one in the settings or with \
             `musicbanana yourspotify add`, or give --from and --token"
        ),
        (n, _) => {
            let mut message = format!(
                "there are {n} YourSpotify connections{whose}; pick one with --connection:"
            );
            for c in &found {
                message.push_str(&format!(
                    "\n  {:>4}  {}/{}  {}",
                    c.id, c.username, c.profile, c.url
                ));
            }
            anyhow::bail!(message)
        }
    }
}

/// Imports from a connection right away, as the server would in its next
/// round: the new plays, or with `all` the whole history again, as after
/// [`refetch`]. Its state is kept as with the server's imports, and the
/// connection is taken the same way, so that the two never import from it at
/// once; while the server imports from it, this fails.
///
/// When `stop` is done first (Ctrl+C), the import stops where it got to and is
/// noted as stopped, so that the server goes on with it in its next round.
pub async fn import_now(
    db: &PgPool,
    id: i64,
    all: bool,
    stop: impl Future<Output = ()>,
) -> anyhow::Result<yourspotify::Report> {
    let taken = sqlx::query_as!(
        Taken,
        r#"UPDATE yourspotify_connection c
              SET started_at = now(),
                  refetch_requested_at = CASE WHEN $2 THEN COALESCE(c.refetch_requested_at, now())
                                              ELSE c.refetch_requested_at END,
                  refetch_at = CASE WHEN $2 AND c.refetch_requested_at IS NULL THEN NULL
                                    ELSE c.refetch_at END,
                  refetch_plays = CASE WHEN $2 AND c.refetch_requested_at IS NULL THEN 0
                                       ELSE c.refetch_plays END,
                  refetch_new = CASE WHEN $2 AND c.refetch_requested_at IS NULL THEN 0
                                     ELSE c.refetch_new END,
                  refetch_left_out = CASE WHEN $2 AND c.refetch_requested_at IS NULL THEN 0
                                          ELSE c.refetch_left_out END
             FROM profile p
             JOIN account a ON a.id = p.account_id
            WHERE p.id = c.profile_id AND c.id = $1
              AND (c.started_at IS NULL
                   OR c.finished_at >= c.started_at
                   OR c.started_at < now() - interval '1 day')
           RETURNING c.id, c.profile_id, c.api_url, c.token, c.finished_at, c.imported,
                     c.refetch_requested_at IS NOT NULL AS "refetch!", c.refetch_at,
                     c.refetch_plays, c.refetch_new, c.refetch_left_out,
                     a.username::text AS "username!", p.slug::text AS "slug!""#,
        id,
        all,
    )
    .fetch_optional(db)
    .await?;
    let Some(taken) = taken else {
        let started = sqlx::query_scalar!(
            "SELECT started_at FROM yourspotify_connection WHERE id = $1",
            id
        )
        .fetch_optional(db)
        .await?;
        match started {
            None => anyhow::bail!("there is no YourSpotify connection {id}"),
            Some(started) => anyhow::bail!(
                "the server is importing from YourSpotify connection {id} right now{}; \
                 try again once it is done (`musicbanana yourspotify list` shows it)",
                started
                    .map(|at| format!(" (since {})", yourspotify::minutes(at)))
                    .unwrap_or_default()
            ),
        }
    };
    tokio::select! {
        biased;
        result = taken.sync(db) => result?,
        () = stop => {
            sqlx::query!(
                "UPDATE yourspotify_connection
                    SET finished_at = now(), error = 'stopped on the command line'
                  WHERE id = $1",
                id,
            )
            .execute(db)
            .await?;
            anyhow::bail!(
                "stopped; the server goes on from there within 15 minutes"
            )
        }
    }
}

/// A connection taken for an import.
struct Taken {
    id: i64,
    profile_id: i64,
    api_url: String,
    token: String,
    finished_at: Option<OffsetDateTime>,
    imported: i64,
    refetch: bool,
    refetch_at: Option<OffsetDateTime>,
    refetch_plays: i64,
    refetch_new: i64,
    refetch_left_out: i64,
    username: String,
    slug: String,
}

impl Taken {
    /// Imports from it and notes how that went. Returns the import's result
    /// within, after noting it; the outer error is one of the database.
    async fn sync(self, db: &PgPool) -> sqlx::Result<anyhow::Result<yourspotify::Report>> {
        let what = format!(
            "YourSpotify connection {} ({}/{})",
            self.id, self.username, self.slug
        );
        let refetch = self.refetch.then_some(Refetch {
            at: self.refetch_at,
            plays: self.refetch_plays,
            new: self.refetch_new,
            left_out: self.refetch_left_out,
        });
        tracing::info!(
            "{what}: fetching {} from {}",
            match (&refetch, self.finished_at) {
                (Some(Refetch { at: Some(at), .. }), _) =>
                    format!("the whole history again, on from {}", at.date()),
                (Some(_), _) => "the whole history again".to_owned(),
                (None, None) => "the history".to_owned(),
                (None, Some(_)) => "new plays".to_owned(),
            },
            self.api_url
        );
        let job = Job {
            id: self.id,
            profile_id: self.profile_id,
            imported: self.imported,
            refetch,
        };
        sync(db, job, &what, &self.api_url, &self.token).await
    }
}

/// An import of one connection, with what it brought before.
struct Job {
    id: i64,
    profile_id: i64,
    imported: i64,
    refetch: Option<Refetch>,
}

/// Fetching the whole history again: how far it got before, and what it found.
struct Refetch {
    at: Option<OffsetDateTime>,
    plays: i64,
    new: i64,
    left_out: i64,
}

async fn sync(
    db: &PgPool,
    job: Job,
    what: &str,
    url: &str,
    token: &str,
) -> sqlx::Result<anyhow::Result<yourspotify::Report>> {
    let Job {
        id,
        profile_id,
        imported,
        refetch,
    } = job;
    // What it brought is noted after each window of time, so that the counts
    // hold when an import stops halfway, and fetching everything again goes on
    // from where it stopped.
    let before = refetch.as_ref().map(|r| (r.plays, r.new, r.left_out));
    let progress = |at: OffsetDateTime, report: yourspotify::Report| {
        let db = db.clone();
        async move {
            let recorded = report.recorded as i64;
            match before {
                None => {
                    sqlx::query!(
                        "UPDATE yourspotify_connection SET imported = $2 WHERE id = $1",
                        id,
                        imported + recorded,
                    )
                    .execute(&db)
                    .await?;
                }
                Some((plays, new, left_out)) => {
                    sqlx::query!(
                        "UPDATE yourspotify_connection
                            SET imported = $2, refetch_at = $3, refetch_plays = $4,
                                refetch_new = $5, refetch_left_out = $6
                          WHERE id = $1",
                        id,
                        imported + recorded,
                        at,
                        plays + report.plays as i64,
                        new + recorded,
                        left_out + report.without_artist as i64,
                    )
                    .execute(&db)
                    .await?;
                }
            }
            Ok(())
        }
    };
    let result = match Source::new(url, token) {
        Ok(source) => {
            let start = match &refetch {
                None => Start::Latest,
                Some(Refetch { at: None, .. }) => Start::Beginning,
                Some(Refetch { at: Some(at), .. }) => Start::At(*at),
            };
            yourspotify::import_from(db, &source, profile_id, start, progress).await
        }
        Err(e) => Err(e),
    };
    let error = match &result {
        Ok(report) => {
            tracing::info!("{what}: {report}");
            None
        }
        Err(e) => {
            tracing::warn!("{what}: {e:#}");
            Some(format!("{e:#}"))
        }
    };
    let refetched = refetch.is_some() && result.is_ok();
    sqlx::query!(
        "UPDATE yourspotify_connection
            SET finished_at = now(), error = $2,
                refetch_requested_at = CASE WHEN $3 THEN NULL ELSE refetch_requested_at END,
                refetch_at = CASE WHEN $3 THEN NULL ELSE refetch_at END,
                refetched_at = CASE WHEN $3 THEN now() ELSE refetched_at END
          WHERE id = $1",
        id,
        error,
        refetched,
    )
    .execute(db)
    .await?;
    Ok(result)
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
