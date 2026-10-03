//! API tokens for scrobble clients.
//!
//! Every client gets its own token, which belongs to one profile: listens sent
//! with it end up there. A token is a random UUID, like a ListenBrainz token, so
//! clients that check the format accept it. It is shown once when created; only
//! its SHA-256 is stored, so the database alone does not give working tokens.

use anyhow::Context;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use time::OffsetDateTime;
use uuid::Uuid;

pub fn hash(token: &str) -> Vec<u8> {
    Sha256::digest(token.trim().as_bytes()).to_vec()
}

pub struct NewToken {
    pub id: i64,
    /// The token itself; it cannot be shown again later.
    pub token: String,
}

/// Creates a token for the profile `slug` of the account `username`.
pub async fn create(
    db: &PgPool,
    username: &str,
    slug: &str,
    label: &str,
) -> anyhow::Result<NewToken> {
    let token = Uuid::new_v4().to_string();
    let id = sqlx::query_scalar!(
        "INSERT INTO api_token (profile_id, token_hash, label)
         SELECT p.id, $3, $4
           FROM profile p
           JOIN account a ON a.id = p.account_id
          WHERE a.username = $1::text::citext AND p.slug = $2::text::citext
         RETURNING id",
        username,
        slug,
        hash(&token),
        label,
    )
    .fetch_optional(db)
    .await?
    .with_context(|| format!("user {username} has no profile {slug}"))?;
    Ok(NewToken { id, token })
}

pub struct TokenInfo {
    pub id: i64,
    pub username: String,
    pub slug: String,
    pub label: String,
    pub created_at: OffsetDateTime,
    pub last_used_at: Option<OffsetDateTime>,
    pub revoked_at: Option<OffsetDateTime>,
}

/// All tokens, or those of one account.
pub async fn list(db: &PgPool, username: Option<&str>) -> sqlx::Result<Vec<TokenInfo>> {
    sqlx::query_as!(
        TokenInfo,
        r#"SELECT t.id, a.username::text AS "username!", p.slug::text AS "slug!", t.label,
                  t.created_at, t.last_used_at, t.revoked_at
             FROM api_token t
             JOIN profile p ON p.id = t.profile_id
             JOIN account a ON a.id = p.account_id
            WHERE $1::text IS NULL OR a.username = $1::text::citext
            ORDER BY t.id"#,
        username,
    )
    .fetch_all(db)
    .await
}

/// Revokes a token. False if there is no such token or it was revoked before.
pub async fn revoke(db: &PgPool, id: i64) -> sqlx::Result<bool> {
    let revoked = sqlx::query!(
        "UPDATE api_token SET revoked_at = now() WHERE id = $1 AND revoked_at IS NULL",
        id,
    )
    .execute(db)
    .await?;
    Ok(revoked.rows_affected() == 1)
}

/// Who a valid token belongs to.
pub struct TokenOwner {
    pub profile_id: i64,
    pub username: String,
}

/// Looks up a token that has not been revoked, and notes that it was used.
pub async fn authenticate(db: &PgPool, token: &str) -> sqlx::Result<Option<TokenOwner>> {
    sqlx::query_as!(
        TokenOwner,
        r#"UPDATE api_token t SET last_used_at = now()
             FROM profile p, account a
            WHERE t.token_hash = $1 AND t.revoked_at IS NULL
              AND p.id = t.profile_id AND a.id = p.account_id
        RETURNING t.profile_id, a.username::text AS "username!""#,
        hash(token),
    )
    .fetch_optional(db)
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_is_sha256_of_the_trimmed_token() {
        // printf 'banana' | sha256sum
        let expected = "b493d48364afe44d11c0165cf470a4164d1e2609911ef998be868d46ade3de4e";
        let hex = |bytes: Vec<u8>| bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
        assert_eq!(hex(hash("banana")), expected);
        assert_eq!(hex(hash(" banana\n")), expected);
    }
}
