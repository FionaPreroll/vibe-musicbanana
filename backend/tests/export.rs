//! Downloading all of an account's data.

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

// fixtures/profiles.sql: Fiona (account 1) with a public default profile and a
// private one, "arbeit"; alex (account 2) with a public default profile.

struct Reply {
    status: StatusCode,
    cookie: Option<String>,
    body: Value,
}

async fn call(
    app: &Router,
    method: Method,
    uri: &str,
    cookie: Option<&str>,
    body: Option<Value>,
) -> Reply {
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
        .map(|value| value.to_str().unwrap().to_owned());
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body = serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into_owned()));
    Reply {
        status,
        cookie,
        body,
    }
}

async fn get(app: &Router, uri: &str, cookie: Option<&str>) -> Reply {
    call(app, Method::GET, uri, cookie, None).await
}

async fn set_password(db: &PgPool, account: i64, password: &str) {
    sqlx::query("UPDATE account SET password_hash = $2 WHERE id = $1")
        .bind(account)
        .bind(auth::hash_password(password).unwrap())
        .execute(db)
        .await
        .unwrap();
}

/// Logs in and returns the cookie to send along.
async fn log_in(app: &Router, login: &str, password: &str) -> String {
    let reply = call(
        app,
        Method::POST,
        "/api/session",
        None,
        Some(json!({"login": login, "password": password})),
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
    let set = reply.cookie.unwrap();
    assert!(
        set.contains("HttpOnly") && set.contains("SameSite=Lax"),
        "{set}"
    );
    set.split(';').next().unwrap().to_owned()
}

fn app(db: PgPool) -> Router {
    router(AppState::new(db), Path::new("does-not-exist"))
}

#[sqlx::test(fixtures("profiles"))]
async fn the_download_holds_everything_but_secrets(db: PgPool) {
    set_password(&db, 1, "banana-split").await;
    let app = app(db.clone());
    let fiona = log_in(&app, "fiona", "banana-split").await;
    for sql in [
        "INSERT INTO api_token (profile_id, token_hash, label) VALUES (1, '\\x0102', 'Navidrome')",
        "INSERT INTO follow (follower_id, profile_id, accepted_at) VALUES (2, 1, now())",
    ] {
        sqlx::query(sql).execute(&db).await.unwrap();
    }
    let tiesto: i64 =
        sqlx::query_scalar("SELECT id FROM listen WHERE profile_id = 1 AND artist_raw = 'Tiësto'")
            .fetch_one(&db)
            .await
            .unwrap();
    let moved = call(
        &app,
        Method::POST,
        "/api/me/profiles/default/trash",
        Some(&fiona),
        Some(json!({ "ids": [tiesto] })),
    )
    .await;
    assert_eq!(moved.status, StatusCode::OK, "{}", moved.body);

    assert_eq!(
        get(&app, "/api/me/export", None).await.status,
        StatusCode::UNAUTHORIZED
    );
    let request = Request::get("/api/me/export")
        .header(header::COOKIE, &fiona)
        .body(Body::empty())
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let disposition = response.headers()[header::CONTENT_DISPOSITION]
        .to_str()
        .unwrap()
        .to_owned();
    assert!(
        disposition.starts_with("attachment; filename=\"musicbanana-Fiona-"),
        "{disposition}"
    );
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let text = String::from_utf8(bytes.to_vec()).unwrap();
    let data: Value = serde_json::from_str(&text).expect("valid JSON");

    assert_eq!(data["account"]["username"], "Fiona");
    assert_eq!(data["account"]["email"], "fiona@example.org");
    assert_eq!(
        [
            &data["account"]["time_zone"],
            &data["account"]["week_start"],
            &data["account"]["show_streaks"]
        ],
        [&Value::Null, &json!(1), &json!(true)]
    );
    assert_eq!(data["followers"][0]["username"], "alex");
    assert_eq!(data["scrobble_tokens"][0]["label"], "Navidrome");
    let profiles = data["profiles"].as_array().unwrap();
    assert_eq!(profiles.len(), 2);
    let default = profiles.iter().find(|p| p["slug"] == "default").unwrap();
    let arbeit = profiles.iter().find(|p| p["slug"] == "arbeit").unwrap();
    assert_eq!(default["listens"].as_array().unwrap().len(), 7);
    assert_eq!(arbeit["listens"].as_array().unwrap().len(), 1);
    assert_eq!(default["trash"].as_array().unwrap().len(), 1);
    assert_eq!(
        default["trash"][0]["track_metadata"]["artist_name"],
        "Tiësto"
    );
    assert!(default["trash"][0]["musicbanana"]["trashed_at"].is_string());

    // ListenBrainz's import format, oldest first.
    let first = &default["listens"][0];
    assert_eq!(first["listened_at"], 1433160000);
    assert_eq!(first["track_metadata"]["artist_name"], "Die Ärzte");
    assert_eq!(first["track_metadata"]["track_name"], "Unrockbar");
    assert_eq!(first["track_metadata"]["release_name"], "Geräusch");
    assert_eq!(first["musicbanana"]["album"], "Geräusch");

    // No secrets.
    for secret in ["$argon2", "password_hash", "token_hash", "unused"] {
        assert!(!text.contains(secret), "{secret}");
    }
}
