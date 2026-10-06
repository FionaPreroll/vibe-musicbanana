pub mod account;
pub mod auth;
pub mod catalog;
pub mod connections;
pub mod delete;
pub mod edit;
mod entities;
mod export;
mod follows;
mod history;
pub mod import_php;
mod listenbrainz;
pub mod merge;
mod profiles;
mod review;
pub mod scrobble;
pub mod search;
pub mod status;
pub mod tokens;
mod week;
pub mod yourspotify;

use std::{net::SocketAddr, path::Path, sync::Arc, time::Instant};

use axum::{
    Json, Router,
    extract::{ConnectInfo, Request, State},
    http::StatusCode,
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
};
use serde::Serialize;
use sqlx::PgPool;
use tower_http::{
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    /// The YourSpotify addresses accounts may connect in their settings.
    pub yourspotify_allowed: Arc<yourspotify::Allowlist>,
    /// Whether each request goes into the log, see [`log_request`].
    pub access_log: bool,
}

impl AppState {
    /// No access log, and no YourSpotify address allowed in the settings.
    pub fn new(db: PgPool) -> Self {
        Self {
            db,
            yourspotify_allowed: Arc::default(),
            access_log: false,
        }
    }
}

/// The whole app: JSON API under `/api`, everything else is the SvelteKit SPA.
pub fn router(state: AppState, static_dir: &Path) -> Router {
    // Unknown paths get index.html so client-side routes survive a reload.
    let spa = ServeDir::new(static_dir).fallback(ServeFile::new(static_dir.join("index.html")));

    let app = Router::new()
        .nest("/api", api())
        .fallback_service(spa)
        .layer(TraceLayer::new_for_http());
    let app = if state.access_log {
        app.layer(middleware::from_fn(log_request))
    } else {
        app
    };
    app.with_state(state)
}

/// One line per request, under the target `musicbanana::access`: client,
/// method, path with query, status and time taken. The client is the first
/// address in X-Forwarded-For when a reverse proxy sends one. Health checks and
/// the frontend's files are left out.
async fn log_request(request: Request, next: Next) -> Response {
    let start = Instant::now();
    let method = request.method().clone();
    let uri = request.uri().clone();
    let forwarded = request
        .headers()
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .map(|v| v.trim().to_owned());
    let client = forwarded.unwrap_or_else(|| {
        request
            .extensions()
            .get::<ConnectInfo<SocketAddr>>()
            .map_or_else(|| "-".into(), |c| c.0.ip().to_string())
    });
    let response = next.run(request).await;
    let path = uri.path();
    if path != "/api/health" && !path.starts_with("/_app/") {
        tracing::info!(
            target: "musicbanana::access",
            "{client} {method} {uri} {} {} ms",
            response.status().as_u16(),
            start.elapsed().as_millis()
        );
    }
    response
}

fn api() -> Router<AppState> {
    Router::new()
        .route("/health", get(health))
        .merge(account::routes())
        .merge(profiles::routes())
        .merge(follows::routes())
        .merge(history::routes())
        .merge(export::routes())
        .merge(entities::routes())
        .merge(search::routes())
        .merge(status::routes())
        .merge(week::routes())
        .merge(review::routes())
        .nest("/listenbrainz/1", listenbrainz::routes())
        .route_layer(middleware::from_fn(status::measure))
        .fallback(|| async { StatusCode::NOT_FOUND })
}

#[derive(Serialize)]
struct Health {
    status: &'static str,
    listens: i64,
}

async fn health(State(state): State<AppState>) -> Result<Json<Health>, AppError> {
    let listens = sqlx::query_scalar!(r#"SELECT count(*) AS "n!" FROM listen"#)
        .fetch_one(&state.db)
        .await?;
    Ok(Json(Health {
        status: "ok",
        listens,
    }))
}

/// What a handler can fail with. Anything unexpected becomes a logged 500.
pub enum AppError {
    NotFound,
    BadRequest(String),
    /// Any other refusal, with its reason.
    Status(StatusCode, String),
    Internal(anyhow::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        match self {
            Self::NotFound => StatusCode::NOT_FOUND.into_response(),
            Self::BadRequest(message) => (StatusCode::BAD_REQUEST, message).into_response(),
            Self::Status(status, message) => (status, message).into_response(),
            Self::Internal(err) => {
                tracing::error!("{err:#}");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal server error").into_response()
            }
        }
    }
}

impl<E: Into<anyhow::Error>> From<E> for AppError {
    fn from(err: E) -> Self {
        Self::Internal(err.into())
    }
}
