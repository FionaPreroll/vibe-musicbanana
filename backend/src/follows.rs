//! Following profiles. Following a public profile takes effect right away; one
//! for followers is a request until its owner says yes, and only then shows the
//! follower the profile. Owners see who follows their profiles and can remove
//! anybody; private profiles can't be followed.

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::{get, put},
};
use serde::Serialize;
use sqlx::PgPool;
use time::OffsetDateTime;

use crate::{AppError, AppState, account::Account};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/profiles/{username}/{slug}/follow",
            get(info).put(follow).delete(unfollow),
        )
        .route("/me/following", get(following))
        .route("/me/followers", get(followers))
        .route(
            "/me/followers/{slug}/{username}",
            put(accept).delete(remove),
        )
}

/// Where the viewer stands with a profile.
#[derive(Serialize)]
struct Info {
    username: String,
    slug: String,
    name: String,
    visibility: String,
    own: bool,
    /// "none", "requested" or "following".
    state: String,
}

/// A profile that can be followed, for any logged-in account: public ones and
/// those for followers, also when the viewer may not see them yet.
async fn find(db: &PgPool, viewer: i64, username: &str, slug: &str) -> Result<Info, AppError> {
    sqlx::query_as!(
        Info,
        r#"SELECT a.username::text AS "username!", p.slug::text AS "slug!", p.name,
                  p.visibility::text AS "visibility!",
                  p.account_id = $3 AS "own!",
                  CASE WHEN f.accepted_at IS NOT NULL THEN 'following'
                       WHEN f.follower_id IS NOT NULL THEN 'requested'
                       ELSE 'none' END AS "state!"
             FROM profile p
             JOIN account a ON a.id = p.account_id
             LEFT JOIN follow f ON f.profile_id = p.id AND f.follower_id = $3
            WHERE a.username = $1::text::citext
              AND p.slug = $2::text::citext
              AND (p.visibility <> 'private' OR p.account_id = $3)"#,
        username,
        slug,
        viewer,
    )
    .fetch_optional(db)
    .await?
    .ok_or(AppError::NotFound)
}

async fn name_of(db: &PgPool, id: i64) -> sqlx::Result<String> {
    sqlx::query_scalar!(
        r#"SELECT username::text AS "username!" FROM account WHERE id = $1"#,
        id
    )
    .fetch_one(db)
    .await
}

async fn info(
    State(state): State<AppState>,
    Account(id): Account,
    Path((username, slug)): Path<(String, String)>,
) -> Result<Json<Info>, AppError> {
    Ok(Json(find(&state.db, id, &username, &slug).await?))
}

async fn follow(
    State(state): State<AppState>,
    Account(id): Account,
    Path((username, slug)): Path<(String, String)>,
) -> Result<Json<Info>, AppError> {
    let profile = find(&state.db, id, &username, &slug).await?;
    if profile.own {
        return Err(AppError::BadRequest("that is your own profile".into()));
    }
    if profile.state == "none" {
        sqlx::query!(
            "INSERT INTO follow (follower_id, profile_id, accepted_at)
             SELECT $1, p.id, CASE WHEN p.visibility = 'public' THEN now() END
               FROM profile p
               JOIN account a ON a.id = p.account_id
              WHERE a.username = $2::text::citext AND p.slug = $3::text::citext
             ON CONFLICT DO NOTHING",
            id,
            profile.username,
            profile.slug,
        )
        .execute(&state.db)
        .await?;
        tracing::info!(
            "{} {} {}/{}",
            name_of(&state.db, id).await?,
            if profile.visibility == "public" {
                "follows"
            } else {
                "asks to follow"
            },
            profile.username,
            profile.slug
        );
    }
    Ok(Json(find(&state.db, id, &username, &slug).await?))
}

/// Stops following, or takes back a request.
async fn unfollow(
    State(state): State<AppState>,
    Account(id): Account,
    Path((username, slug)): Path<(String, String)>,
) -> Result<StatusCode, AppError> {
    // Also for profiles that went private meanwhile.
    let removed = sqlx::query!(
        "DELETE FROM follow f
          USING profile p, account a
          WHERE f.profile_id = p.id AND a.id = p.account_id AND f.follower_id = $1
            AND a.username = $2::text::citext AND p.slug = $3::text::citext",
        id,
        username,
        slug,
    )
    .execute(&state.db)
    .await?;
    if removed.rows_affected() > 0 {
        tracing::info!(
            "{} no longer follows {username}/{slug}",
            name_of(&state.db, id).await?
        );
    }
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Serialize)]
struct Followed {
    username: String,
    slug: String,
    name: String,
    visibility: String,
    state: String,
    #[serde(with = "time::serde::rfc3339")]
    since: OffsetDateTime,
}

/// The profiles the viewer follows or asked to follow, except private ones.
async fn following(
    State(state): State<AppState>,
    Account(id): Account,
) -> Result<Json<Vec<Followed>>, AppError> {
    let followed = sqlx::query_as!(
        Followed,
        r#"SELECT a.username::text AS "username!", p.slug::text AS "slug!", p.name,
                  p.visibility::text AS "visibility!",
                  CASE WHEN f.accepted_at IS NULL THEN 'requested' ELSE 'following' END
                      AS "state!",
                  coalesce(f.accepted_at, f.created_at) AS "since!"
             FROM follow f
             JOIN profile p ON p.id = f.profile_id
             JOIN account a ON a.id = p.account_id
            WHERE f.follower_id = $1 AND p.visibility <> 'private'
            ORDER BY f.accepted_at IS NULL, a.username, p.slug"#,
        id,
    )
    .fetch_all(&state.db)
    .await?;
    Ok(Json(followed))
}

#[derive(Serialize)]
struct Follower {
    /// The slug of the viewer's profile.
    profile: String,
    username: String,
    state: String,
    #[serde(with = "time::serde::rfc3339")]
    since: OffsetDateTime,
}

/// Who follows the viewer's profiles or asks to, the requests first.
async fn followers(
    State(state): State<AppState>,
    Account(id): Account,
) -> Result<Json<Vec<Follower>>, AppError> {
    let followers = sqlx::query_as!(
        Follower,
        r#"SELECT p.slug::text AS "profile!", a.username::text AS "username!",
                  CASE WHEN f.accepted_at IS NULL THEN 'requested' ELSE 'following' END
                      AS "state!",
                  coalesce(f.accepted_at, f.created_at) AS "since!"
             FROM follow f
             JOIN profile p ON p.id = f.profile_id
             JOIN account a ON a.id = f.follower_id
            WHERE p.account_id = $1
            ORDER BY f.accepted_at IS NOT NULL, p.slug, a.username"#,
        id,
    )
    .fetch_all(&state.db)
    .await?;
    Ok(Json(followers))
}

/// Says yes to a request.
async fn accept(
    State(state): State<AppState>,
    Account(id): Account,
    Path((slug, username)): Path<(String, String)>,
) -> Result<StatusCode, AppError> {
    let accepted = sqlx::query!(
        "UPDATE follow f SET accepted_at = coalesce(f.accepted_at, now())
           FROM profile p, account a
          WHERE f.profile_id = p.id AND a.id = f.follower_id AND p.account_id = $1
            AND p.slug = $2::text::citext AND a.username = $3::text::citext",
        id,
        slug,
        username,
    )
    .execute(&state.db)
    .await?;
    if accepted.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    tracing::info!(
        "{} lets {username} follow the profile {slug}",
        name_of(&state.db, id).await?
    );
    Ok(StatusCode::NO_CONTENT)
}

/// Removes a follower, or says no to a request.
async fn remove(
    State(state): State<AppState>,
    Account(id): Account,
    Path((slug, username)): Path<(String, String)>,
) -> Result<StatusCode, AppError> {
    let removed = sqlx::query!(
        "DELETE FROM follow f
          USING profile p, account a
          WHERE f.profile_id = p.id AND a.id = f.follower_id AND p.account_id = $1
            AND p.slug = $2::text::citext AND a.username = $3::text::citext",
        id,
        slug,
        username,
    )
    .execute(&state.db)
    .await?;
    if removed.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    tracing::info!(
        "{} removed {username} from the followers of the profile {slug}",
        name_of(&state.db, id).await?
    );
    Ok(StatusCode::NO_CONTENT)
}
