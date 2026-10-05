//! Logins in the browser and the settings behind them: the viewer's profiles,
//! the API tokens of their scrobble clients and their password.
//!
//! A login sets a cookie with a random token, of which only the SHA-256 is
//! stored. It lasts 30 days from the last request. Requests that change
//! something send JSON, which a form on another site cannot, and the cookie is
//! SameSite=Lax; together they keep other sites from acting with the viewer's
//! login.

use std::{
    collections::HashMap,
    sync::{LazyLock, Mutex},
    time::{Duration, Instant},
};

use axum::{
    Json, Router,
    extract::{FromRequestParts, Path, State},
    http::{HeaderMap, HeaderValue, StatusCode, header, request::Parts},
    response::{IntoResponse, Response},
    routing::{delete, get, patch, post, put},
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{
    AppError, AppState, auth,
    connections::{self, Connection},
    tokens,
};

const COOKIE: &str = "musicbanana_session";

/// Days a login lasts after the last request.
const LIFETIME_DAYS: i32 = 30;

/// The user name of account `id`, for the log lines of changes.
async fn name_of(db: &PgPool, id: i64) -> sqlx::Result<String> {
    sqlx::query_scalar!(
        r#"SELECT username::text AS "username!" FROM account WHERE id = $1"#,
        id
    )
    .fetch_one(db)
    .await
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/session", post(login).delete(logout))
        .route("/me", get(me))
        .route("/me/password", put(change_password))
        .route("/me/username", put(change_username))
        .route("/me/profiles", post(create_profile))
        .route("/me/profiles/{slug}", patch(update_profile))
        .route("/me/tokens", get(list_tokens).post(create_token))
        .route("/me/tokens/{id}", delete(revoke_token))
        .route(
            "/me/yourspotify",
            get(list_connections).post(add_connection),
        )
        .route("/me/yourspotify/{id}", delete(remove_connection))
        .route("/me/yourspotify/allowed", get(allowed_addresses))
}

/// The account logged in with the request's cookie, if any.
#[derive(Clone, Copy, Debug)]
pub struct Viewer(pub Option<i64>);

impl FromRequestParts<AppState> for Viewer {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, AppError> {
        let Some(token) = session_cookie(&parts.headers) else {
            return Ok(Self(None));
        };
        // Each request moves the end of the login on.
        let account = sqlx::query_scalar!(
            "UPDATE session SET expires_at = now() + make_interval(days => $2)
              WHERE token_hash = $1 AND expires_at > now()
              RETURNING account_id",
            tokens::hash(&token),
            LIFETIME_DAYS,
        )
        .fetch_optional(&state.db)
        .await?;
        Ok(Self(account))
    }
}

/// A logged-in account; requests without a login get 401.
pub struct Account(pub i64);

impl FromRequestParts<AppState> for Account {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, AppError> {
        Viewer::from_request_parts(parts, state)
            .await?
            .0
            .map(Self)
            .ok_or_else(|| AppError::Status(StatusCode::UNAUTHORIZED, "not logged in".into()))
    }
}

fn session_cookie(headers: &HeaderMap) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(';'))
        .find_map(|pair| {
            let (name, value) = pair.trim().split_once('=')?;
            (name == COOKIE && !value.is_empty()).then(|| value.to_owned())
        })
}

/// The cookie, marked Secure when a proxy in front says the request came over HTTPS.
fn set_cookie(headers: &HeaderMap, value: &str, max_age_days: i32) -> HeaderValue {
    let secure = headers
        .get("x-forwarded-proto")
        .is_some_and(|proto| proto.as_bytes().eq_ignore_ascii_case(b"https"));
    let max_age = i64::from(max_age_days) * 24 * 3600;
    let cookie = format!(
        "{COOKIE}={value}; Path=/; Max-Age={max_age}; HttpOnly; SameSite=Lax{}",
        if secure { "; Secure" } else { "" }
    );
    HeaderValue::from_str(&cookie).expect("the cookie is ASCII")
}

/// Failed logins per name: after `LIMIT` within `WINDOW`, the name is refused
/// until the window has passed, which makes guessing a password slow.
mod throttle {
    use super::*;

    const LIMIT: u32 = 10;
    const WINDOW: Duration = Duration::from_secs(15 * 60);

    static FAILED: LazyLock<Mutex<HashMap<String, (u32, Instant)>>> = LazyLock::new(Mutex::default);

    pub fn locked(name: &str) -> bool {
        let mut failed = FAILED.lock().unwrap();
        failed.retain(|_, (_, since)| since.elapsed() < WINDOW);
        failed.get(name).is_some_and(|(count, _)| *count >= LIMIT)
    }

    pub fn failed(name: &str) {
        let mut failed = FAILED.lock().unwrap();
        failed
            .entry(name.to_owned())
            .or_insert((0, Instant::now()))
            .0 += 1;
    }

    pub fn succeeded(name: &str) {
        FAILED.lock().unwrap().remove(name);
    }
}

#[derive(Deserialize)]
struct Credentials {
    /// User name or email address.
    login: String,
    password: String,
}

#[derive(Serialize)]
struct LoggedIn {
    username: String,
}

/// Checks a password off the async threads, as argon2 takes a while on purpose.
async fn verify(stored: String, legacy_md5: bool, password: String) -> anyhow::Result<bool> {
    Ok(
        tokio::task::spawn_blocking(move || auth::verify_password(&stored, legacy_md5, &password))
            .await?,
    )
}

async fn hash(password: String) -> anyhow::Result<String> {
    tokio::task::spawn_blocking(move || auth::hash_password(&password)).await?
}

/// Checked against when there is no such account, so that the time a failed
/// login takes does not tell which names exist.
static DUMMY_HASH: LazyLock<String> =
    LazyLock::new(|| auth::hash_password("no such account").expect("hashing works"));

async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(credentials): Json<Credentials>,
) -> Result<Response, AppError> {
    let name = credentials.login.trim().to_lowercase();
    if throttle::locked(&name) {
        tracing::warn!("login refused for {name:?}: too many failed logins");
        return Err(AppError::Status(
            StatusCode::TOO_MANY_REQUESTS,
            "too many failed logins; try again in 15 minutes".into(),
        ));
    }
    let account = sqlx::query!(
        r#"SELECT id, username::text AS "username!", password_hash, password_legacy_md5
             FROM account
            WHERE username = $1::text::citext OR email = $1::text::citext"#,
        credentials.login.trim(),
    )
    .fetch_optional(&state.db)
    .await?;
    let (stored, legacy) = account.as_ref().map_or_else(
        || (DUMMY_HASH.clone(), false),
        |a| (a.password_hash.clone(), a.password_legacy_md5),
    );
    let right = verify(stored, legacy, credentials.password.clone()).await?;
    let Some(account) = account.filter(|_| right) else {
        tracing::warn!("failed login for {name:?}");
        throttle::failed(&name);
        return Err(AppError::Status(
            StatusCode::UNAUTHORIZED,
            "wrong user name or password".into(),
        ));
    };
    throttle::succeeded(&name);

    // The MD5 of musicbanana-php gives way to a proper hash now that the
    // password is known.
    if account.password_legacy_md5 {
        let new = hash(credentials.password).await?;
        sqlx::query!(
            "UPDATE account SET password_hash = $2, password_legacy_md5 = false WHERE id = $1",
            account.id,
            new,
        )
        .execute(&state.db)
        .await?;
    }

    let token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    sqlx::query!(
        "INSERT INTO session (token_hash, account_id, expires_at)
         VALUES ($1, $2, now() + make_interval(days => $3))",
        tokens::hash(&token),
        account.id,
        LIFETIME_DAYS,
    )
    .execute(&state.db)
    .await?;
    let cookie = set_cookie(&headers, &token, LIFETIME_DAYS);
    tracing::info!("{} logged in", account.username);
    Ok((
        [(header::SET_COOKIE, cookie)],
        Json(LoggedIn {
            username: account.username,
        }),
    )
        .into_response())
}

async fn logout(State(state): State<AppState>, headers: HeaderMap) -> Result<Response, AppError> {
    if let Some(token) = session_cookie(&headers) {
        let ended = sqlx::query_scalar!(
            r#"DELETE FROM session s USING account a
                WHERE s.token_hash = $1 AND a.id = s.account_id
               RETURNING a.username::text AS "username!""#,
            tokens::hash(&token)
        )
        .fetch_optional(&state.db)
        .await?;
        if let Some(username) = ended {
            tracing::info!("{username} logged out");
        }
    }
    let cookie = set_cookie(&headers, "", 0);
    Ok((StatusCode::NO_CONTENT, [(header::SET_COOKIE, cookie)]).into_response())
}

#[derive(Serialize)]
struct Me {
    username: String,
    email: String,
    /// Whether the account sees the status page.
    admin: bool,
    profiles: Vec<OwnProfile>,
}

#[derive(Serialize)]
struct OwnProfile {
    slug: String,
    name: String,
    visibility: String,
    listens: i64,
}

async fn own_profiles(db: &PgPool, account: i64) -> sqlx::Result<Vec<OwnProfile>> {
    sqlx::query_as!(
        OwnProfile,
        r#"SELECT p.slug::text AS "slug!", p.name, p.visibility::text AS "visibility!",
                  (SELECT count(*) FROM listen l WHERE l.profile_id = p.id) AS "listens!"
             FROM profile p
            WHERE p.account_id = $1
            ORDER BY p.slug <> 'default', p.slug"#,
        account,
    )
    .fetch_all(db)
    .await
}

async fn me(State(state): State<AppState>, Account(id): Account) -> Result<Json<Me>, AppError> {
    let account = sqlx::query!(
        r#"SELECT username::text AS "username!", email::text AS "email!", is_admin
             FROM account WHERE id = $1"#,
        id,
    )
    .fetch_one(&state.db)
    .await?;
    Ok(Json(Me {
        username: account.username,
        email: account.email,
        admin: account.is_admin,
        profiles: own_profiles(&state.db, id).await?,
    }))
}

#[derive(Deserialize)]
struct PasswordChange {
    current: String,
    new: String,
}

/// The shortest password taken.
pub const MIN_PASSWORD: usize = 8;

/// Sets a new password and ends the account's other logins.
async fn change_password(
    State(state): State<AppState>,
    Account(id): Account,
    headers: HeaderMap,
    Json(change): Json<PasswordChange>,
) -> Result<StatusCode, AppError> {
    let account = sqlx::query!(
        "SELECT password_hash, password_legacy_md5 FROM account WHERE id = $1",
        id
    )
    .fetch_one(&state.db)
    .await?;
    if !verify(
        account.password_hash,
        account.password_legacy_md5,
        change.current,
    )
    .await?
    {
        return Err(AppError::Status(
            StatusCode::FORBIDDEN,
            "the current password is wrong".into(),
        ));
    }
    if change.new.chars().count() < MIN_PASSWORD {
        return Err(AppError::BadRequest(format!(
            "the new password needs at least {MIN_PASSWORD} characters"
        )));
    }
    let new = hash(change.new).await?;
    let token = session_cookie(&headers).unwrap_or_default();
    let mut tx = state.db.begin().await?;
    sqlx::query!(
        "UPDATE account SET password_hash = $2, password_legacy_md5 = false WHERE id = $1",
        id,
        new,
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!(
        "DELETE FROM session WHERE account_id = $1 AND token_hash <> $2",
        id,
        tokens::hash(&token),
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    tracing::info!(
        "{} changed the password; other logins ended",
        name_of(&state.db, id).await?
    );
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
enum Visibility {
    Public,
    Followers,
    Private,
}

impl Visibility {
    fn as_str(self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Followers => "followers",
            Self::Private => "private",
        }
    }
}

#[derive(Deserialize)]
struct NewProfile {
    slug: String,
    name: String,
    visibility: Option<Visibility>,
}

#[derive(Deserialize)]
struct UsernameChange {
    username: String,
}

#[derive(Serialize)]
struct Renamed {
    username: String,
}

/// Renames the account; links with the old name lead to the new one.
async fn change_username(
    State(state): State<AppState>,
    Account(id): Account,
    Json(change): Json<UsernameChange>,
) -> Result<Json<Renamed>, AppError> {
    match rename(&state.db, id, &change.username).await? {
        Ok((old, username)) => {
            tracing::info!("{old} renamed the account to {username}");
            Ok(Json(Renamed { username }))
        }
        Err(refusal) => Err(AppError::BadRequest(refusal)),
    }
}

/// Letters, digits, dashes, dots and underscores, as it goes into the address
/// /u/<name>, starting with a letter or digit.
pub fn check_username(name: &str) -> Result<(), String> {
    let fine = (1..=32).contains(&name.chars().count())
        && name
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '-' | '.' | '_'))
        && name.chars().next().is_some_and(char::is_alphanumeric);
    if fine {
        Ok(())
    } else {
        Err(format!(
            "\"{name}\" does not work as a user name: use up to 32 letters, digits, dashes, \
             dots and underscores, starting with a letter or digit"
        ))
    }
}

/// Renames account `id` to `new`, returning the old and the new name, or why
/// not. The old name is kept for the links that use it (see
/// migrations/0010_former_usernames.sql); nobody else can take it, and neither
/// the old names of others.
pub async fn rename(
    db: &PgPool,
    id: i64,
    new: &str,
) -> anyhow::Result<Result<(String, String), String>> {
    let new = new.trim();
    if let Err(refusal) = check_username(new) {
        return Ok(Err(refusal));
    }
    let mut tx = db.begin().await?;
    // One rename at a time, so two can't swap names into each other's old ones.
    sqlx::query!("LOCK TABLE former_username IN SHARE ROW EXCLUSIVE MODE")
        .execute(&mut *tx)
        .await?;
    let old = sqlx::query_scalar!(
        r#"SELECT username::text AS "username!" FROM account WHERE id = $1 FOR UPDATE"#,
        id
    )
    .fetch_one(&mut *tx)
    .await?;
    let taken = sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT FROM account WHERE username = $2::text::citext AND id <> $1)
               OR EXISTS (SELECT FROM former_username
                           WHERE username = $2::text::citext AND account_id <> $1)
               AS "taken!""#,
        id,
        new,
    )
    .fetch_one(&mut *tx)
    .await?;
    if taken {
        return Ok(Err(format!("the user name \"{new}\" is taken")));
    }
    if old == new {
        return Ok(Ok((old, new.to_owned())));
    }
    sqlx::query!(
        "DELETE FROM former_username WHERE username = $1::text::citext",
        new
    )
    .execute(&mut *tx)
    .await?;
    // A change of case only keeps the name.
    if old.to_lowercase() != new.to_lowercase() {
        sqlx::query!(
            "INSERT INTO former_username (username, account_id) VALUES ($1, $2)",
            old,
            id
        )
        .execute(&mut *tx)
        .await?;
    }
    sqlx::query!("UPDATE account SET username = $2 WHERE id = $1", id, new)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Ok((old, new.to_owned())))
}

/// Lower-case letters, digits and dashes, as it goes into the address. The names
/// of entry pages are taken: /u/<name>/artist/12 is an artist of the default profile.
pub fn check_slug(slug: &str) -> Result<(), String> {
    let fine = (1..=32).contains(&slug.len())
        && slug
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && !slug.starts_with('-');
    if !fine {
        return Err(format!(
            "\"{slug}\" does not work as a profile address: use up to 32 lower-case \
             letters, digits and dashes"
        ));
    }
    if matches!(slug, "artist" | "album" | "track" | "search" | "follow") {
        return Err(format!("\"{slug}\" is taken by the pages of a profile"));
    }
    Ok(())
}

async fn create_profile(
    State(state): State<AppState>,
    Account(id): Account,
    Json(new): Json<NewProfile>,
) -> Result<(StatusCode, Json<Vec<OwnProfile>>), AppError> {
    let slug = new.slug.trim();
    check_slug(slug).map_err(AppError::BadRequest)?;
    let name = new.name.trim();
    if name.is_empty() {
        return Err(AppError::BadRequest("the profile needs a name".into()));
    }
    let visibility = new.visibility.unwrap_or(Visibility::Public);
    let created = sqlx::query!(
        "INSERT INTO profile (account_id, slug, name, visibility)
         VALUES ($1, $2, $3, $4::text::visibility)
         ON CONFLICT (account_id, slug) DO NOTHING",
        id,
        slug,
        name,
        visibility.as_str(),
    )
    .execute(&state.db)
    .await?;
    if created.rows_affected() == 0 {
        return Err(AppError::Status(
            StatusCode::CONFLICT,
            format!("you have a profile \"{slug}\" already"),
        ));
    }
    tracing::info!(
        "{} created the profile {slug} ({:?}, {})",
        name_of(&state.db, id).await?,
        name,
        visibility.as_str()
    );
    Ok((
        StatusCode::CREATED,
        Json(own_profiles(&state.db, id).await?),
    ))
}

#[derive(Deserialize)]
struct ProfileChange {
    name: Option<String>,
    visibility: Option<Visibility>,
}

async fn update_profile(
    State(state): State<AppState>,
    Account(id): Account,
    Path(slug): Path<String>,
    Json(change): Json<ProfileChange>,
) -> Result<Json<Vec<OwnProfile>>, AppError> {
    let name = change.name.as_deref().map(str::trim);
    if name == Some("") {
        return Err(AppError::BadRequest("the profile needs a name".into()));
    }
    let changed = sqlx::query!(
        "UPDATE profile
            SET name = COALESCE($3, name),
                visibility = COALESCE($4::text::visibility, visibility)
          WHERE account_id = $1 AND slug = $2::text::citext",
        id,
        slug,
        name,
        change.visibility.map(Visibility::as_str),
    )
    .execute(&state.db)
    .await?;
    if changed.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    let owner = name_of(&state.db, id).await?;
    if let Some(name) = name {
        tracing::info!("{owner} named the profile {slug} {name:?}");
    }
    if let Some(visibility) = change.visibility {
        tracing::info!("{owner} made the profile {slug} {}", visibility.as_str());
    }
    // Everyone may see a public profile, so the open requests are answered.
    if let Some(Visibility::Public) = change.visibility {
        sqlx::query!(
            "UPDATE follow f SET accepted_at = now()
               FROM profile p
              WHERE f.profile_id = p.id AND f.accepted_at IS NULL
                AND p.account_id = $1 AND p.slug = $2::text::citext",
            id,
            slug,
        )
        .execute(&state.db)
        .await?;
    }
    Ok(Json(own_profiles(&state.db, id).await?))
}

#[derive(Serialize)]
struct Token {
    id: i64,
    profile: String,
    label: String,
    #[serde(with = "time::serde::rfc3339")]
    created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    last_used_at: Option<OffsetDateTime>,
}

async fn list_tokens(
    State(state): State<AppState>,
    Account(id): Account,
) -> Result<Json<Vec<Token>>, AppError> {
    let tokens = sqlx::query_as!(
        Token,
        r#"SELECT t.id, p.slug::text AS "profile!", t.label, t.created_at, t.last_used_at
             FROM api_token t
             JOIN profile p ON p.id = t.profile_id
            WHERE p.account_id = $1 AND t.revoked_at IS NULL
            ORDER BY t.created_at DESC, t.id DESC"#,
        id,
    )
    .fetch_all(&state.db)
    .await?;
    Ok(Json(tokens))
}

#[derive(Deserialize)]
struct NewToken {
    profile: String,
    label: String,
}

#[derive(Serialize)]
struct CreatedToken {
    id: i64,
    /// Shown this once.
    token: String,
}

async fn create_token(
    State(state): State<AppState>,
    Account(id): Account,
    Json(new): Json<NewToken>,
) -> Result<(StatusCode, Json<CreatedToken>), AppError> {
    let label = new.label.trim();
    if label.is_empty() {
        return Err(AppError::BadRequest(
            "name what the token is for, e.g. Navidrome".into(),
        ));
    }
    let username = sqlx::query_scalar!(
        r#"SELECT username::text AS "username!" FROM account WHERE id = $1"#,
        id
    )
    .fetch_one(&state.db)
    .await?;
    let created = tokens::create(&state.db, &username, &new.profile, label)
        .await
        .map_err(|_| AppError::NotFound)?;
    tracing::info!(
        "{username} created scrobble token {} {label:?} for the profile {}",
        created.id,
        new.profile
    );
    Ok((
        StatusCode::CREATED,
        Json(CreatedToken {
            id: created.id,
            token: created.token,
        }),
    ))
}

async fn revoke_token(
    State(state): State<AppState>,
    Account(id): Account,
    Path(token): Path<i64>,
) -> Result<StatusCode, AppError> {
    let revoked = sqlx::query!(
        "UPDATE api_token SET revoked_at = now()
          WHERE id = $1 AND revoked_at IS NULL
            AND profile_id IN (SELECT id FROM profile WHERE account_id = $2)",
        token,
        id,
    )
    .execute(&state.db)
    .await?;
    if revoked.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    tracing::info!(
        "{} revoked scrobble token {token}",
        name_of(&state.db, id).await?
    );
    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------- YourSpotify

/// The account's YourSpotify connections; the tokens stay on the server.
async fn list_connections(
    State(state): State<AppState>,
    Account(id): Account,
) -> Result<Json<Vec<Connection>>, AppError> {
    Ok(Json(connections::list(&state.db, Some(id)).await?))
}

#[derive(Deserialize)]
struct NewConnection {
    profile: String,
    url: String,
    token: String,
}

/// Connects a YourSpotify account to one of the account's profiles, once
/// YourSpotify has taken the token; the server imports from it within a minute.
async fn add_connection(
    State(state): State<AppState>,
    Account(id): Account,
    Json(new): Json<NewConnection>,
) -> Result<(StatusCode, Json<Vec<Connection>>), AppError> {
    let profile = sqlx::query_scalar!(
        "SELECT id FROM profile WHERE account_id = $1 AND slug = $2::text::citext",
        id,
        new.profile,
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;
    let owner = name_of(&state.db, id).await?;
    let allowed = &state.yourspotify_allowed;
    if !allowed.allows(&new.url) {
        tracing::warn!(
            "{owner} tried to connect the profile {} to YourSpotify at {:?}, which is not allowed",
            new.profile,
            new.url
        );
        return Err(AppError::BadRequest(if allowed.is_empty() {
            "this server takes YourSpotify connections from its command line only".into()
        } else {
            format!(
                "this server takes YourSpotify at {} only",
                allowed.addresses().join(" or ")
            )
        }));
    }
    let connection = connections::add(&state.db, profile, &new.url, &new.token)
        .await
        .map_err(|e| AppError::BadRequest(format!("{e:#}")))?;
    tracing::info!(
        "{owner} connected the profile {} to YourSpotify at {} (connection {connection})",
        new.profile,
        new.url.trim()
    );
    Ok((
        StatusCode::CREATED,
        Json(connections::list(&state.db, Some(id)).await?),
    ))
}

async fn remove_connection(
    State(state): State<AppState>,
    Account(account): Account,
    Path(id): Path<i64>,
) -> Result<StatusCode, AppError> {
    if connections::remove(&state.db, id, Some(account)).await? {
        tracing::info!(
            "{} removed YourSpotify connection {id}",
            name_of(&state.db, account).await?
        );
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound)
    }
}

/// The YourSpotify addresses the settings take, see [`crate::yourspotify::Allowlist`].
async fn allowed_addresses(
    State(state): State<AppState>,
    Account(_): Account,
) -> Json<Vec<String>> {
    Json(state.yourspotify_allowed.addresses())
}
