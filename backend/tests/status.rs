//! The status page for admins.

use std::path::Path;

use axum::{
    Router,
    body::Body,
    http::{Method, Request, StatusCode, header},
};
use http_body_util::BodyExt;
use musicbanana::{AppState, auth, router};
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;

async fn call(
    app: &Router,
    method: Method,
    uri: &str,
    cookie: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value, Option<String>) {
    let mut request = Request::builder().method(method).uri(uri);
    if let Some(cookie) = cookie {
        request = request.header(header::COOKIE, cookie);
    }
    let request = match body {
        Some(body) => request
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string())),
        None => request.body(Body::empty()),
    }
    .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let cookie = response
        .headers()
        .get(header::SET_COOKIE)
        .map(|v| v.to_str().unwrap().split(';').next().unwrap().to_owned());
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body = serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into_owned()));
    (status, body, cookie)
}

#[sqlx::test(fixtures("profiles"))]
async fn the_status_is_for_admins(db: PgPool) {
    sqlx::query("UPDATE account SET password_hash = $1")
        .bind(auth::hash_password("banana-split").unwrap())
        .execute(&db)
        .await
        .unwrap();
    let app = router(AppState::new(db.clone()), Path::new("does-not-exist"));
    let log_in = |name: &str| {
        let app = app.clone();
        let body = json!({"login": name, "password": "banana-split"});
        async move {
            call(&app, Method::POST, "/api/session", None, Some(body))
                .await
                .2
                .unwrap()
        }
    };
    let fiona = log_in("fiona").await;
    let alex = log_in("alex").await;

    let status = "/api/admin/status";
    assert_eq!(
        call(&app, Method::GET, status, None, None).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(&app, Method::GET, status, Some(&fiona), None).await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        call(&app, Method::GET, "/api/me", Some(&fiona), None)
            .await
            .1["admin"],
        false
    );

    sqlx::query("UPDATE account SET is_admin = true WHERE id = 1")
        .execute(&db)
        .await
        .unwrap();
    assert_eq!(
        call(&app, Method::GET, "/api/me", Some(&fiona), None)
            .await
            .1["admin"],
        true
    );
    assert_eq!(
        call(&app, Method::GET, status, Some(&alex), None).await.0,
        StatusCode::FORBIDDEN
    );
    let (code, body, _) = call(&app, Method::GET, status, Some(&fiona), None).await;
    assert_eq!(code, StatusCode::OK, "{body}");
    assert_eq!(body["build"]["version"], env!("CARGO_PKG_VERSION"));
    assert!(body["database"]["bytes"].as_i64().unwrap() > 0);
    assert_eq!(
        body["database"]["migration"],
        json!(sqlx::migrate!().iter().map(|m| m.version).max())
    );
    let tables: Vec<&str> = body["database"]["tables"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert!(tables.contains(&"listen"), "{tables:?}");
    assert!(
        body["workers"]["yourspotify"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    // The requests above, by route; timings are kept for the whole process,
    // which other tests share, so only what this one did for sure.
    let routes = body["routes"].as_array().unwrap();
    let me = routes.iter().find(|r| r["route"] == "GET /api/me").unwrap();
    assert!(me["requests"].as_u64().unwrap() >= 2, "{me}");
    assert!(routes.iter().any(|r| r["route"] == "POST /api/session"));
}
