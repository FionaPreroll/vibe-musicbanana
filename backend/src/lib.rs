pub mod auth;
pub mod catalog;
pub mod edit;
mod entities;
pub mod import_php;
mod listenbrainz;
pub mod merge;
mod profiles;
pub mod scrobble;
pub mod tokens;

use std::path::Path;

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
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
}

/// The whole app: JSON API under `/api`, everything else is the SvelteKit SPA.
pub fn router(state: AppState, static_dir: &Path) -> Router {
    // Unknown paths get index.html so client-side routes survive a reload.
    let spa = ServeDir::new(static_dir).fallback(ServeFile::new(static_dir.join("index.html")));

    Router::new()
        .nest("/api", api())
        .fallback_service(spa)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

fn api() -> Router<AppState> {
    Router::new()
        .route("/health", get(health))
        .merge(profiles::routes())
        .merge(entities::routes())
        .nest("/listenbrainz/1", listenbrainz::routes())
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
    Internal(anyhow::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        match self {
            Self::NotFound => StatusCode::NOT_FOUND.into_response(),
            Self::BadRequest(message) => (StatusCode::BAD_REQUEST, message).into_response(),
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
