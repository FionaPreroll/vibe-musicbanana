//! Deleting a profile or a whole account with everything that belongs to it,
//! from the command line only: `musicbanana profile delete` and
//! `musicbanana account delete`. Artists, albums and tracks stay in the shared
//! catalog; the listens, tokens, connections, follows and logins go.

use std::fmt;

use anyhow::{Context, bail};
use sqlx::{PgPool, Postgres, Transaction};

/// What a deletion takes away, or would.
#[derive(Debug, Default)]
pub struct Removal {
    pub profiles: Vec<String>,
    pub listens: i64,
    pub tokens: i64,
    pub connections: i64,
    pub followers: i64,
}

impl fmt::Display for Removal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "profiles: {}; listens: {}; scrobble tokens: {}; YourSpotify connections: {}; \
             followers: {}",
            self.profiles.join(", "),
            self.listens,
            self.tokens,
            self.connections,
            self.followers
        )
    }
}

async fn account_id(tx: &mut Transaction<'_, Postgres>, username: &str) -> anyhow::Result<i64> {
    sqlx::query_scalar!(
        "SELECT id FROM account WHERE username = $1::text::citext FOR UPDATE",
        username
    )
    .fetch_optional(&mut **tx)
    .await?
    .with_context(|| format!("there is no account {username}"))
}

/// Counts what goes with the profiles, and takes their listens out of the
/// journal of merges, so that undoing a merge later doesn't bring them back.
async fn remove_profiles(
    tx: &mut Transaction<'_, Postgres>,
    profiles: &[i64],
) -> anyhow::Result<Removal> {
    let counts = sqlx::query!(
        r#"SELECT (SELECT count(*) FROM listen WHERE profile_id = ANY($1)) AS "listens!",
                  (SELECT count(*) FROM api_token
                    WHERE profile_id = ANY($1) AND revoked_at IS NULL) AS "tokens!",
                  (SELECT count(*) FROM yourspotify_connection
                    WHERE profile_id = ANY($1)) AS "connections!",
                  (SELECT count(*) FROM follow WHERE profile_id = ANY($1)) AS "followers!",
                  coalesce((SELECT array_agg(slug::text ORDER BY slug) FROM profile
                             WHERE id = ANY($1)), '{}') AS "slugs!""#,
        profiles,
    )
    .fetch_one(&mut **tx)
    .await?;
    sqlx::query!(
        "DELETE FROM merge_change c
          USING listen l
          WHERE c.tbl = 'listen' AND (c.key ->> 'id')::bigint = l.id
            AND l.profile_id = ANY($1)",
        profiles,
    )
    .execute(&mut **tx)
    .await?;
    sqlx::query!("DELETE FROM profile WHERE id = ANY($1)", profiles)
        .execute(&mut **tx)
        .await?;
    Ok(Removal {
        profiles: counts.slugs,
        listens: counts.listens,
        tokens: counts.tokens,
        connections: counts.connections,
        followers: counts.followers,
    })
}

/// Deletes the profile `slug` of `username` with its listens, or with
/// `really` false only says what would go. The default profile goes with the
/// account only.
pub async fn profile(
    db: &PgPool,
    username: &str,
    slug: &str,
    really: bool,
) -> anyhow::Result<Removal> {
    let mut tx = db.begin().await?;
    let account = account_id(&mut tx, username).await?;
    let id = sqlx::query_scalar!(
        "SELECT id FROM profile WHERE account_id = $1 AND slug = $2::text::citext",
        account,
        slug,
    )
    .fetch_optional(&mut *tx)
    .await?
    .with_context(|| format!("{username} has no profile {slug}"))?;
    if slug.eq_ignore_ascii_case("default") {
        bail!(
            "the default profile goes with the account only: musicbanana account delete {username}"
        );
    }
    let removal = remove_profiles(&mut tx, &[id]).await?;
    if really {
        tx.commit().await?;
    }
    Ok(removal)
}

/// Deletes the account `username` with all its profiles and listens, or with
/// `really` false only says what would go. Its user name is free again.
pub async fn account(db: &PgPool, username: &str, really: bool) -> anyhow::Result<Removal> {
    let mut tx = db.begin().await?;
    let account = account_id(&mut tx, username).await?;
    let profiles = sqlx::query_scalar!("SELECT id FROM profile WHERE account_id = $1", account)
        .fetch_all(&mut *tx)
        .await?;
    let removal = remove_profiles(&mut tx, &profiles).await?;
    // Logins, follows of other profiles and old names go with the account.
    sqlx::query!("DELETE FROM account WHERE id = $1", account)
        .execute(&mut *tx)
        .await?;
    if really {
        tx.commit().await?;
    }
    Ok(removal)
}
