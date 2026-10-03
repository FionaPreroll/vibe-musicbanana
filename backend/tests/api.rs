use std::path::Path;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use musicbanana::{AppState, router};
use sqlx::PgPool;
use tower::ServiceExt;

// #[sqlx::test] creates a fresh database per test and runs ./migrations on it.

#[sqlx::test]
async fn health_counts_listens(db: PgPool) {
    let app = router(AppState { db }, Path::new("does-not-exist"));

    let res = app
        .oneshot(Request::get("/api/health").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json, serde_json::json!({ "status": "ok", "listens": 0 }));
}

#[sqlx::test]
async fn unknown_api_route_is_404(db: PgPool) {
    let app = router(AppState { db }, Path::new("does-not-exist"));

    let res = app
        .oneshot(Request::get("/api/nope").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}
