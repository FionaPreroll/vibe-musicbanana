//! Following profiles, and asking to follow one for followers.

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

async fn send(app: &Router, method: Method, uri: &str, cookie: &str) -> Reply {
    call(app, method, uri, Some(cookie), None).await
}

#[sqlx::test(fixtures("profiles"))]
async fn following_needs_the_owners_yes_for_followers_only(db: PgPool) {
    set_password(&db, 1, "banana-split").await;
    set_password(&db, 2, "kiwi-kiwi").await;
    sqlx::query("UPDATE profile SET visibility = 'followers' WHERE id = 3")
        .execute(&db)
        .await
        .unwrap();
    let app = app(db.clone());
    let fiona = log_in(&app, "fiona", "banana-split").await;
    let alex = log_in(&app, "alex", "kiwi-kiwi").await;

    // Alex's profile is for followers: Fiona sees it only after asking and Alex's yes.
    assert_eq!(
        get(&app, "/api/profiles/alex/default", Some(&fiona))
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    let info = get(&app, "/api/profiles/alex/default/follow", Some(&fiona)).await;
    assert_eq!(info.status, StatusCode::OK, "{}", info.body);
    assert_eq!(info.body["visibility"], "followers");
    assert_eq!(info.body["state"], "none");
    // Not for anonymous viewers, and not for private profiles.
    assert_eq!(
        get(&app, "/api/profiles/alex/default/follow", None)
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        get(&app, "/api/profiles/fiona/arbeit/follow", Some(&alex))
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        send(
            &app,
            Method::PUT,
            "/api/profiles/fiona/arbeit/follow",
            &alex
        )
        .await
        .status,
        StatusCode::NOT_FOUND
    );

    let asked = send(
        &app,
        Method::PUT,
        "/api/profiles/alex/default/follow",
        &fiona,
    )
    .await;
    assert_eq!(asked.status, StatusCode::OK, "{}", asked.body);
    assert_eq!(asked.body["state"], "requested");
    assert_eq!(
        get(&app, "/api/profiles/alex/default", Some(&fiona))
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    let following = get(&app, "/api/me/following", Some(&fiona)).await;
    assert_eq!(following.body[0]["username"], "alex");
    assert_eq!(following.body[0]["state"], "requested");

    let requests = get(&app, "/api/me/followers", Some(&alex)).await;
    assert_eq!(requests.body.as_array().unwrap().len(), 1);
    assert_eq!(requests.body[0]["username"], "Fiona");
    assert_eq!(requests.body[0]["profile"], "default");
    assert_eq!(requests.body[0]["state"], "requested");
    // Only the owner says yes.
    assert_eq!(
        send(&app, Method::PUT, "/api/me/followers/default/fiona", &fiona)
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        send(&app, Method::PUT, "/api/me/followers/default/fiona", &alex)
            .await
            .status,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        get(&app, "/api/profiles/alex/default", Some(&fiona))
            .await
            .status,
        StatusCode::OK
    );
    assert_eq!(
        get(&app, "/api/profiles/alex/default/follow", Some(&fiona))
            .await
            .body["state"],
        "following"
    );

    // Removed by the owner, Fiona no longer sees it.
    assert_eq!(
        send(
            &app,
            Method::DELETE,
            "/api/me/followers/default/Fiona",
            &alex
        )
        .await
        .status,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        get(&app, "/api/profiles/alex/default", Some(&fiona))
            .await
            .status,
        StatusCode::NOT_FOUND
    );

    // A request is answered when the profile goes public.
    send(
        &app,
        Method::PUT,
        "/api/profiles/alex/default/follow",
        &fiona,
    )
    .await;
    let public = call(
        &app,
        Method::PATCH,
        "/api/me/profiles/default",
        Some(&alex),
        Some(json!({"visibility": "public"})),
    )
    .await;
    assert_eq!(public.status, StatusCode::OK, "{}", public.body);
    assert_eq!(
        get(&app, "/api/me/followers", Some(&alex)).await.body[0]["state"],
        "following"
    );
}

#[sqlx::test(fixtures("profiles"))]
async fn following_a_public_profile_and_stopping(db: PgPool) {
    set_password(&db, 2, "kiwi-kiwi").await;
    let app = app(db.clone());
    let alex = log_in(&app, "alex", "kiwi-kiwi").await;

    let followed = send(
        &app,
        Method::PUT,
        "/api/profiles/Fiona/default/follow",
        &alex,
    )
    .await;
    assert_eq!(followed.status, StatusCode::OK, "{}", followed.body);
    assert_eq!(followed.body["state"], "following");
    // Twice changes nothing.
    let again = send(
        &app,
        Method::PUT,
        "/api/profiles/fiona/default/follow",
        &alex,
    )
    .await;
    assert_eq!(again.body["state"], "following");
    assert_eq!(
        get(&app, "/api/me/following", Some(&alex))
            .await
            .body
            .as_array()
            .unwrap()
            .len(),
        1
    );
    // Not one's own profile.
    assert_eq!(
        send(
            &app,
            Method::PUT,
            "/api/profiles/alex/default/follow",
            &alex
        )
        .await
        .status,
        StatusCode::BAD_REQUEST
    );

    assert_eq!(
        send(
            &app,
            Method::DELETE,
            "/api/profiles/fiona/default/follow",
            &alex
        )
        .await
        .status,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        get(&app, "/api/profiles/fiona/default/follow", Some(&alex))
            .await
            .body["state"],
        "none"
    );
    assert_eq!(
        get(&app, "/api/me/following", Some(&alex)).await.body,
        json!([])
    );
}
