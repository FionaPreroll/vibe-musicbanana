//! Logging in, private profiles and the settings of the logged-in account.

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
async fn the_old_password_logs_in_and_gets_a_new_hash(db: PgPool) {
    // As the PHP import stores it: the MD5 of "banana", wrapped in argon2id.
    sqlx::query("UPDATE account SET password_hash = $1, password_legacy_md5 = true WHERE id = 1")
        .bind(auth::wrap_legacy_md5("72b302bf297a228a75730123efef7c41").unwrap())
        .execute(&db)
        .await
        .unwrap();
    let app = app(db.clone());

    for (login, password) in [("Fiona", "apple"), ("nobody", "banana")] {
        let reply = call(
            &app,
            Method::POST,
            "/api/session",
            None,
            Some(json!({"login": login, "password": password})),
        )
        .await;
        assert_eq!(reply.status, StatusCode::UNAUTHORIZED, "{login}");
        assert_eq!(reply.body, "wrong user name or password");
        assert!(reply.cookie.is_none());
    }
    assert_eq!(
        get(&app, "/api/me", None).await.status,
        StatusCode::UNAUTHORIZED
    );

    let cookie = log_in(&app, "fiona", "banana").await;
    let me = get(&app, "/api/me", Some(&cookie)).await;
    assert_eq!(me.status, StatusCode::OK);
    assert_eq!(me.body["username"], "Fiona");
    assert_eq!(
        me.body["profiles"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| (
                p["slug"].as_str().unwrap(),
                p["visibility"].as_str().unwrap()
            ))
            .collect::<Vec<_>>(),
        [("default", "public"), ("arbeit", "private")]
    );
    let legacy: bool = sqlx::query_scalar("SELECT password_legacy_md5 FROM account WHERE id = 1")
        .fetch_one(&db)
        .await
        .unwrap();
    assert!(!legacy);
    // The upgraded hash takes the same password, and so does the email address.
    log_in(&app, "fiona@example.org", "banana").await;

    // Logging out ends the login.
    let reply = call(&app, Method::DELETE, "/api/session", Some(&cookie), None).await;
    assert_eq!(reply.status, StatusCode::NO_CONTENT);
    assert!(reply.cookie.unwrap().contains("Max-Age=0"));
    assert_eq!(
        get(&app, "/api/me", Some(&cookie)).await.status,
        StatusCode::UNAUTHORIZED
    );
}

#[sqlx::test(fixtures("profiles"))]
async fn a_private_profile_is_there_for_its_owner_only(db: PgPool) {
    set_password(&db, 1, "banana-split").await;
    set_password(&db, 2, "kiwi-kiwi").await;
    let app = app(db.clone());
    let fiona = log_in(&app, "Fiona", "banana-split").await;
    let alex = log_in(&app, "alex", "kiwi-kiwi").await;

    let arbeit = "/api/profiles/Fiona/arbeit";
    assert_eq!(get(&app, arbeit, None).await.status, StatusCode::NOT_FOUND);
    assert_eq!(
        get(&app, arbeit, Some(&alex)).await.status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        get(&app, &format!("{arbeit}/listens"), Some(&alex))
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    let own = get(&app, arbeit, Some(&fiona)).await;
    assert_eq!(own.status, StatusCode::OK);
    assert_eq!(
        (&own.body["visibility"], &own.body["own"]),
        (&json!("private"), &json!(true))
    );
    let other = get(&app, "/api/profiles/alex/default", Some(&fiona)).await;
    assert_eq!(other.body["own"], false);

    let listed = |reply: Reply| -> Vec<String> {
        reply
            .body
            .as_array()
            .unwrap()
            .iter()
            .map(|p| {
                format!(
                    "{}/{}",
                    p["username"].as_str().unwrap(),
                    p["slug"].as_str().unwrap()
                )
            })
            .collect()
    };
    assert_eq!(
        listed(get(&app, "/api/profiles", None).await),
        ["alex/default", "Fiona/default"]
    );
    assert_eq!(
        listed(get(&app, "/api/profiles", Some(&fiona)).await),
        ["alex/default", "Fiona/arbeit", "Fiona/default"]
    );

    // For followers: alex follows the profile, nobody else sees it.
    sqlx::query("UPDATE profile SET visibility = 'followers' WHERE id = 2")
        .execute(&db)
        .await
        .unwrap();
    sqlx::query("INSERT INTO follow (follower_id, profile_id) VALUES (2, 2)")
        .execute(&db)
        .await
        .unwrap();
    assert_eq!(get(&app, arbeit, Some(&alex)).await.status, StatusCode::OK);
    assert_eq!(get(&app, arbeit, None).await.status, StatusCode::NOT_FOUND);
    assert_eq!(
        get(&app, "/api/profiles/Fiona/arbeit/artists/1", Some(&alex))
            .await
            .status,
        StatusCode::OK
    );
}

#[sqlx::test(fixtures("profiles"))]
async fn profiles_tokens_and_password_in_the_settings(db: PgPool) {
    set_password(&db, 1, "banana-split").await;
    set_password(&db, 2, "kiwi-kiwi").await;
    let app = app(db.clone());
    let fiona = log_in(&app, "Fiona", "banana-split").await;
    let post = |uri: &'static str, body: Value| {
        let (app, fiona) = (app.clone(), fiona.clone());
        async move { call(&app, Method::POST, uri, Some(&fiona), Some(body)).await }
    };

    // A new profile, then made private.
    let reply = post(
        "/api/me/profiles",
        json!({"slug": "spotify", "name": "Spotify"}),
    )
    .await;
    assert_eq!(reply.status, StatusCode::CREATED, "{}", reply.body);
    for (slug, status) in [
        ("spotify", StatusCode::CONFLICT),
        ("Spotify!", StatusCode::BAD_REQUEST),
        ("artist", StatusCode::BAD_REQUEST),
    ] {
        let reply = post("/api/me/profiles", json!({"slug": slug, "name": "X"})).await;
        assert_eq!(reply.status, status, "{slug}: {}", reply.body);
    }
    let reply = call(
        &app,
        Method::PATCH,
        "/api/me/profiles/spotify",
        Some(&fiona),
        Some(json!({"visibility": "private"})),
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(
        get(&app, "/api/profiles/Fiona/spotify", None).await.status,
        StatusCode::NOT_FOUND
    );

    // A token for a scrobble client, shown once, then revoked.
    let reply = post(
        "/api/me/tokens",
        json!({"profile": "spotify", "label": "Navidrome"}),
    )
    .await;
    assert_eq!(reply.status, StatusCode::CREATED);
    let token = reply.body["token"].as_str().unwrap().to_owned();
    let id = reply.body["id"].as_i64().unwrap();
    let tokens = get(&app, "/api/me/tokens", Some(&fiona)).await.body;
    assert_eq!(tokens[0]["profile"], "spotify");
    assert_eq!(tokens[0]["label"], "Navidrome");
    assert!(tokens[0].get("token").is_none());
    let validate = Request::get("/api/listenbrainz/1/validate-token")
        .header(header::AUTHORIZATION, format!("Token {token}"))
        .body(Body::empty())
        .unwrap();
    let valid = app.clone().oneshot(validate).await.unwrap();
    assert_eq!(valid.status(), StatusCode::OK);
    let alex = log_in(&app, "alex", "kiwi-kiwi").await;
    let uri = format!("/api/me/tokens/{id}");
    assert_eq!(
        call(&app, Method::DELETE, &uri, Some(&alex), None)
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(&app, Method::DELETE, &uri, Some(&fiona), None)
            .await
            .status,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        get(&app, "/api/me/tokens", Some(&fiona)).await.body,
        json!([])
    );

    // A new password ends the other logins but not this one.
    let other = log_in(&app, "Fiona", "banana-split").await;
    let change = |current: &str, new: &str| {
        let (app, fiona) = (app.clone(), fiona.clone());
        let body = json!({"current": current, "new": new});
        async move {
            call(
                &app,
                Method::PUT,
                "/api/me/password",
                Some(&fiona),
                Some(body),
            )
            .await
            .status
        }
    };
    assert_eq!(change("wrong", "long enough").await, StatusCode::FORBIDDEN);
    assert_eq!(
        change("banana-split", "short").await,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        change("banana-split", "long enough").await,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        get(&app, "/api/me", Some(&fiona)).await.status,
        StatusCode::OK
    );
    assert_eq!(
        get(&app, "/api/me", Some(&other)).await.status,
        StatusCode::UNAUTHORIZED
    );
    log_in(&app, "Fiona", "long enough").await;
}

#[sqlx::test(fixtures("profiles"))]
async fn other_sites_cannot_act_with_the_login(db: PgPool) {
    set_password(&db, 1, "banana-split").await;
    let app = app(db);
    let fiona = log_in(&app, "Fiona", "banana-split").await;
    // A form on another site can only send form data, which is refused.
    let form = Request::post("/api/me/profiles")
        .header(header::COOKIE, &fiona)
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from("slug=evil&name=Evil"))
        .unwrap();
    let reply = app.clone().oneshot(form).await.unwrap();
    assert_eq!(reply.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
}

#[sqlx::test(fixtures("profiles"))]
async fn guessing_a_password_is_slowed_down(db: PgPool) {
    sqlx::query("UPDATE account SET username = 'guessed' WHERE id = 2")
        .execute(&db)
        .await
        .unwrap();
    set_password(&db, 2, "the-right-one").await;
    let app = app(db);
    let attempt = |password: &'static str| {
        let app = app.clone();
        async move {
            call(
                &app,
                Method::POST,
                "/api/session",
                None,
                Some(json!({"login": "guessed", "password": password})),
            )
            .await
            .status
        }
    };
    for _ in 0..10 {
        assert_eq!(attempt("wrong").await, StatusCode::UNAUTHORIZED);
    }
    assert_eq!(
        attempt("the-right-one").await,
        StatusCode::TOO_MANY_REQUESTS
    );
}

#[sqlx::test(fixtures("profiles"))]
async fn renaming_keeps_the_old_links(db: PgPool) {
    set_password(&db, 1, "banana-split").await;
    set_password(&db, 2, "kiwi-kiwi").await;
    let app = app(db.clone());
    let fiona = log_in(&app, "Fiona", "banana-split").await;
    let alex = log_in(&app, "alex", "kiwi-kiwi").await;
    let rename = |cookie: String, name: &str| {
        let app = app.clone();
        let body = json!({ "username": name });
        async move {
            call(
                &app,
                Method::PUT,
                "/api/me/username",
                Some(&cookie),
                Some(body),
            )
            .await
        }
    };

    // Another account's name, and one that would not work in an address.
    for (name, refusal) in [("ALEX", "taken"), ("fi/ona", "does not work")] {
        let reply = rename(fiona.clone(), name).await;
        assert_eq!(reply.status, StatusCode::BAD_REQUEST);
        assert!(
            reply.body.as_str().unwrap().contains(refusal),
            "{}",
            reply.body
        );
    }

    let reply = rename(fiona.clone(), "fiona-banana").await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
    assert_eq!(reply.body["username"], "fiona-banana");
    assert_eq!(
        get(&app, "/api/me", Some(&fiona)).await.body["username"],
        "fiona-banana"
    );
    assert_eq!(
        get(&app, "/api/profiles/fiona-banana/default", None)
            .await
            .status,
        StatusCode::OK
    );
    assert_eq!(
        get(&app, "/api/profiles/Fiona/default", None).await.status,
        StatusCode::NOT_FOUND
    );

    // The old name leads to the new one, but only to profiles the viewer may see.
    let moved = get(&app, "/api/renamed/fiona/default", None).await;
    assert_eq!(moved.status, StatusCode::OK);
    assert_eq!(moved.body["username"], "fiona-banana");
    for cookie in [None, Some(alex.as_str())] {
        assert_eq!(
            get(&app, "/api/renamed/Fiona/arbeit", cookie).await.status,
            StatusCode::NOT_FOUND
        );
    }
    assert_eq!(
        get(&app, "/api/renamed/Fiona/arbeit", Some(&fiona))
            .await
            .status,
        StatusCode::OK
    );
    assert_eq!(
        get(&app, "/api/renamed/alex/default", None).await.status,
        StatusCode::NOT_FOUND
    );

    // Nobody else can take the old name, its owner can; logging in takes the new one.
    let reply = rename(alex.clone(), "Fiona").await;
    assert_eq!(reply.status, StatusCode::BAD_REQUEST);
    assert!(reply.body.as_str().unwrap().contains("taken"));
    log_in(&app, "fiona-banana", "banana-split").await;
    let reply = rename(fiona.clone(), "Fiona").await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
    assert_eq!(
        get(&app, "/api/renamed/fiona-banana/default", None)
            .await
            .body["username"],
        "Fiona"
    );
    assert_eq!(
        get(&app, "/api/renamed/Fiona/default", None).await.status,
        StatusCode::NOT_FOUND
    );
}
