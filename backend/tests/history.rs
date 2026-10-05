//! "Edit listening history": the trash for wrong listens.

use std::path::Path;

use axum::{
    Router,
    body::Body,
    http::{Method, Request, StatusCode, header},
};
use http_body_util::BodyExt;
use musicbanana::{
    AppState, auth,
    catalog::Mbids,
    router,
    scrobble::{self, Listen},
};
use serde_json::{Value, json};
use sqlx::PgPool;
use time::OffsetDateTime;
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

async fn post(app: &Router, uri: &str, cookie: &str, body: Value) -> Reply {
    call(app, Method::POST, uri, Some(cookie), Some(body)).await
}

fn ids(page: &Value) -> Vec<i64> {
    page["listens"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["id"].as_i64().unwrap())
        .collect()
}

const HISTORY: &str = "/api/me/profiles/default/history";
const TRASH: &str = "/api/me/profiles/default/trash";

#[sqlx::test(fixtures("profiles"))]
async fn wrong_listens_go_to_the_trash_and_come_back(db: PgPool) {
    set_password(&db, 1, "banana-split").await;
    set_password(&db, 2, "kiwi-kiwi").await;
    let app = app(db.clone());
    let fiona = log_in(&app, "fiona", "banana-split").await;
    let alex = log_in(&app, "alex", "kiwi-kiwi").await;

    let all = get(&app, HISTORY, Some(&fiona)).await;
    assert_eq!(all.status, StatusCode::OK, "{}", all.body);
    assert_eq!(all.body["total"], 8);
    // Every word, without accents, in the artist, track or album.
    let found = get(
        &app,
        &format!("{HISTORY}?q=arzte%20UNROCKBAR"),
        Some(&fiona),
    )
    .await;
    assert_eq!(found.body["total"], 3);
    assert_eq!(found.body["listens"][0]["artist"], "Die Ärzte");
    // Paged.
    let first = get(&app, &format!("{HISTORY}?limit=5"), Some(&fiona)).await;
    assert_eq!(ids(&first.body).len(), 5);
    let next = first.body["next"].as_str().unwrap().replace('+', "%2B");
    let second = get(
        &app,
        &format!("{HISTORY}?limit=5&before={next}"),
        Some(&fiona),
    )
    .await;
    assert_eq!(ids(&second.body).len(), 3);
    assert!(second.body["next"].is_null());
    // Nobody else's.
    assert_eq!(
        get(&app, HISTORY, Some(&alex)).await.status,
        StatusCode::OK,
        "alex's own"
    );
    assert_eq!(
        get(&app, "/api/me/profiles/arbeit/history", Some(&alex))
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        get(&app, HISTORY, None).await.status,
        StatusCode::UNAUTHORIZED
    );

    // Two by their ids: gone from the profile, in the trash.
    let picked = ids(&found.body)[..2].to_vec();
    let moved = post(&app, TRASH, &fiona, json!({ "ids": picked })).await;
    assert_eq!(moved.status, StatusCode::OK, "{}", moved.body);
    assert_eq!(moved.body["ids"].as_array().unwrap().len(), 2);
    // Another account's listens can't be picked.
    let other: i64 = sqlx::query_scalar("SELECT id FROM listen WHERE profile_id = 3")
        .fetch_one(&db)
        .await
        .unwrap();
    let moved = post(&app, TRASH, &fiona, json!({ "ids": [other] })).await;
    assert_eq!(moved.body["ids"], json!([]));
    let overview = get(&app, "/api/profiles/fiona/default", None).await;
    assert_eq!(overview.body["listens"], 6);
    let trash = get(&app, TRASH, Some(&fiona)).await;
    assert_eq!(trash.body["total"], 2);
    assert!(trash.body["listens"][0]["trashed_at"].is_string());

    // Sending one again (a scrobbler, an import) doesn't bring it back.
    let at: OffsetDateTime =
        sqlx::query_scalar("SELECT listened_at FROM listen_trash ORDER BY id LIMIT 1")
            .fetch_one(&db)
            .await
            .unwrap();
    let again = Listen {
        listened_at: at,
        artist: "Die Ärzte".into(),
        track: "Unrockbar".into(),
        album: None,
        track_number: None,
        duration_ms: None,
        client: None,
        mbids: Mbids::default(),
        extra: None,
    };
    assert_eq!(scrobble::record(&db, 1, &[again]).await.unwrap(), 0);

    // Restored with their ids.
    let restored = post(&app, &format!("{TRASH}/restore"), &fiona, json!({})).await;
    assert_eq!(restored.status, StatusCode::OK, "{}", restored.body);
    assert_eq!(restored.body, json!({"restored": 2, "kept": 0}));
    let back: Vec<i64> = sqlx::query_scalar("SELECT id FROM listen WHERE id = ANY($1)")
        .bind(&picked)
        .fetch_all(&db)
        .await
        .unwrap();
    assert_eq!(back.len(), 2);
    assert_eq!(get(&app, TRASH, Some(&fiona)).await.body["total"], 0);
}

#[sqlx::test(fixtures("profiles"))]
async fn a_filter_selects_and_the_trash_empties(db: PgPool) {
    set_password(&db, 1, "banana-split").await;
    let app = app(db.clone());
    let fiona = log_in(&app, "fiona", "banana-split").await;

    // Not everything at once.
    let refused = post(&app, TRASH, &fiona, json!({})).await;
    assert_eq!(refused.status, StatusCode::BAD_REQUEST);

    // A day in March 2016.
    let day = json!({"from": "2016-03-01T00:00:00Z", "to": "2016-03-02T23:59:59Z"});
    let moved = post(&app, TRASH, &fiona, day).await;
    assert_eq!(
        moved.body["ids"].as_array().unwrap().len(),
        2,
        "{}",
        moved.body
    );
    let moved = post(&app, TRASH, &fiona, json!({"q": "tiesto"})).await;
    assert_eq!(moved.body["ids"].as_array().unwrap().len(), 1);
    let tiesto = moved.body["ids"][0].as_i64().unwrap();

    // Tiësto merged into Die Ärzte meanwhile: the listen comes back as the latter's.
    sqlx::query("UPDATE artist SET merged_into = 1 WHERE id = 3")
        .execute(&db)
        .await
        .unwrap();
    // Another listen at the time of one of the March listens keeps it in the trash.
    sqlx::query(
        "INSERT INTO listen (profile_id, listened_at, artist_raw, track_raw, artist_id, recording_id)
         VALUES (1, '2016-03-01 12:00:00+00', 'Björk', 'Jóga', 2, 4)",
    )
    .execute(&db)
    .await
    .unwrap();
    let restored = post(&app, &format!("{TRASH}/restore"), &fiona, json!({})).await;
    assert_eq!(restored.body, json!({"restored": 2, "kept": 1}));
    let artist: i64 = sqlx::query_scalar("SELECT artist_id FROM listen WHERE id = $1")
        .bind(tiesto)
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(artist, 1);

    let emptied = post(&app, &format!("{TRASH}/empty"), &fiona, json!({})).await;
    assert_eq!(emptied.body, json!({"deleted": 1}));
    assert_eq!(get(&app, TRASH, Some(&fiona)).await.body["total"], 0);
}
