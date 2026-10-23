//! Comparing two profiles.

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

// fixtures/profiles.sql: Fiona's public default profile heard Die Ärzte four
// times, Björk three times and Tiësto once in 2015 and 2016 (Tiësto on New
// Year's Eve 2015, 23:30 UTC). Her private profile "arbeit" heard Tiësto once,
// alex's public default profile "Deine Schuld" by Die Ärzte once, without album.

const FIONA_ALEX: &str = "/api/compare/fiona/default/alex/default";
const FIONA_ARBEIT: &str = "/api/compare/fiona/default/fiona/arbeit";

async fn call(
    app: &Router,
    method: Method,
    uri: &str,
    cookie: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Option<String>, Value) {
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
    (status, cookie, body)
}

async fn get(app: &Router, uri: &str, cookie: Option<&str>) -> (StatusCode, Value) {
    let (status, _, body) = call(app, Method::GET, uri, cookie, None).await;
    (status, body)
}

async fn get_ok(app: &Router, uri: &str, cookie: Option<&str>) -> Value {
    let (status, body) = get(app, uri, cookie).await;
    assert_eq!(status, StatusCode::OK, "GET {uri}: {body}");
    body
}

async fn log_in(app: &Router, db: &PgPool, account: i64, login: &str) -> String {
    sqlx::query("UPDATE account SET password_hash = $2 WHERE id = $1")
        .bind(account)
        .bind(auth::hash_password("banana").unwrap())
        .execute(db)
        .await
        .unwrap();
    let (status, cookie, body) = call(
        app,
        Method::POST,
        "/api/session",
        None,
        Some(json!({"login": login, "password": "banana"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    cookie.unwrap().split(';').next().unwrap().to_owned()
}

fn app(db: PgPool) -> Router {
    router(AppState::new(db), Path::new("does-not-exist"))
}

/// `[name, a, b]` per shared entry, with the artist for albums and tracks.
fn shared(json: &Value, list: &str) -> Vec<Value> {
    json[list]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| match e.get("artist") {
            Some(artist) => json!([e["name"], artist, e["a"], e["b"]]),
            None => json!([e["name"], e["a"], e["b"]]),
        })
        .collect()
}

/// `[name, listens]` of the artists only one side heard.
fn only(side: &Value) -> Vec<Value> {
    side["only"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| json!([e["name"], e["listens"]]))
        .collect()
}

#[sqlx::test(fixtures("profiles"))]
async fn compares_two_public_profiles(db: PgPool) {
    let app = app(db);
    let c = get_ok(&app, FIONA_ALEX, None).await;

    assert_eq!(c["a"]["username"], "Fiona");
    assert_eq!(c["b"]["username"], "alex");
    assert_eq!([&c["a"]["listens"], &c["a"]["artists"]], [8, 3]);
    assert_eq!([&c["b"]["listens"], &c["b"]["artists"]], [1, 1]);

    // Die Ärzte are half of Fiona's listens and all of alex's.
    assert_eq!(c["score"], 0.5);
    assert_eq!(
        c["shared"],
        json!({"artists": 1, "releases": 0, "recordings": 1})
    );
    assert_eq!(shared(&c, "artists"), [json!(["Die Ärzte", 4, 1])]);
    assert_eq!(shared(&c, "releases"), Vec::<Value>::new());
    assert_eq!(
        shared(&c, "recordings"),
        [json!(["Deine Schuld", "Die Ärzte", 1, 1])]
    );

    assert_eq!(c["a"]["only_count"], 2);
    assert_eq!(only(&c["a"]), [json!(["Björk", 3]), json!(["Tiësto", 1])]);
    assert_eq!(c["b"]["only_count"], 0);
    assert_eq!(only(&c["b"]), Vec::<Value>::new());

    // The other way round, the sides swap and the score stays.
    let c = get_ok(&app, "/api/compare/ALEX/default/fiona/default", None).await;
    assert_eq!(c["a"]["username"], "alex");
    assert_eq!(c["score"], 0.5);
    assert_eq!(shared(&c, "artists"), [json!(["Die Ärzte", 1, 4])]);
    assert_eq!(only(&c["b"]), [json!(["Björk", 3]), json!(["Tiësto", 1])]);
}

#[sqlx::test(fixtures("profiles"))]
async fn compares_a_period(db: PgPool) {
    let app = app(db);

    // alex heard nothing in 2015. Die Ärzte are no "only" artist all the same:
    // alex heard them, just later.
    let c = get_ok(&app, &format!("{FIONA_ALEX}?year=2015"), None).await;
    assert_eq!([&c["a"]["listens"], &c["b"]["listens"]], [4, 0]);
    assert_eq!(c["score"], 0.0);
    assert_eq!(shared(&c, "artists"), Vec::<Value>::new());
    assert_eq!(only(&c["a"]), [json!(["Björk", 1]), json!(["Tiësto", 1])]);

    // In Berlin, Tiësto's New Year's Eve listen is already 2016.
    let c = get_ok(
        &app,
        &format!("{FIONA_ALEX}?year=2015&tz=Europe/Berlin"),
        None,
    )
    .await;
    assert_eq!(only(&c["a"]), [json!(["Björk", 1])]);

    let c = get_ok(&app, &format!("{FIONA_ALEX}?from=2016-01-01&limit=1"), None).await;
    assert_eq!([&c["a"]["listens"], &c["b"]["listens"]], [4, 1]);
    // Die Ärzte: min(2/4, 1/1).
    assert_eq!(c["score"], 0.5);
    assert_eq!(c["a"]["only_count"], 1);
    assert_eq!(only(&c["a"]), [json!(["Björk", 2])]);

    // By source: the fixture's listens name no client.
    let c = get_ok(&app, &format!("{FIONA_ALEX}?source=navidrome"), None).await;
    assert_eq!([&c["a"]["listens"], &c["b"]["listens"]], [0, 0]);
    assert_eq!(c["score"], 0.0);
}

#[sqlx::test(fixtures("profiles"))]
async fn ranks_by_shared_share(db: PgPool) {
    // alex also hears Björk a lot and Tiësto once: Björk is then a bigger
    // share of both profiles than Die Ärzte.
    sqlx::query(
        "INSERT INTO listen (profile_id, listened_at, artist_raw, track_raw, artist_id, recording_id, release_id)
         SELECT 3, '2016-05-01 12:00Z'::timestamptz + make_interval(days => i), 'Björk', 'Human Behaviour', 2, 3, 2
           FROM generate_series(1, 6) i
         UNION ALL
         SELECT 3, '2016-06-01 12:00Z', 'Tiësto', 'Adagio for Strings', 3, 5, NULL",
    )
    .execute(&db)
    .await
    .unwrap();

    let c = get_ok(&app(db), FIONA_ALEX, None).await;
    assert_eq!(c["b"]["listens"], 8);
    assert_eq!(
        shared(&c, "artists"),
        [
            json!(["Björk", 3, 6]),
            json!(["Die Ärzte", 4, 1]),
            json!(["Tiësto", 1, 1])
        ]
    );
    assert_eq!(shared(&c, "releases"), [json!(["Debut", "Björk", 2, 6])]);
    // 3/8 + 1/8 + 1/8
    assert_eq!(c["score"], 0.625);
    assert_eq!(c["shared"]["artists"], 3);
    assert_eq!(c["a"]["only_count"], 0);
}

#[sqlx::test(fixtures("profiles"))]
async fn needs_both_profiles_visible(db: PgPool) {
    let app = app(db.clone());

    // Fiona's private profile is hers only.
    assert_eq!(get(&app, FIONA_ARBEIT, None).await.0, StatusCode::NOT_FOUND);
    let alex = log_in(&app, &db, 2, "alex").await;
    assert_eq!(
        get(&app, FIONA_ARBEIT, Some(&alex)).await.0,
        StatusCode::NOT_FOUND
    );
    let reversed = "/api/compare/fiona/arbeit/alex/default";
    assert_eq!(
        get(&app, reversed, Some(&alex)).await.0,
        StatusCode::NOT_FOUND
    );

    let fiona = log_in(&app, &db, 1, "fiona").await;
    let c = get_ok(&app, FIONA_ARBEIT, Some(&fiona)).await;
    assert_eq!(c["b"]["name"], "Arbeit");
    assert_eq!(shared(&c, "artists"), [json!(["Tiësto", 1, 1])]);
    assert_eq!(c["score"], 0.125);

    // A profile for followers: only with a follow the owner said yes to.
    sqlx::query("UPDATE profile SET visibility = 'followers' WHERE id = 3")
        .execute(&db)
        .await
        .unwrap();
    assert_eq!(get(&app, FIONA_ALEX, None).await.0, StatusCode::NOT_FOUND);
    sqlx::query("INSERT INTO follow (follower_id, profile_id) VALUES (1, 3)")
        .execute(&db)
        .await
        .unwrap();
    assert_eq!(
        get(&app, FIONA_ALEX, Some(&fiona)).await.0,
        StatusCode::NOT_FOUND
    );
    sqlx::query("UPDATE follow SET accepted_at = now()")
        .execute(&db)
        .await
        .unwrap();
    get_ok(&app, FIONA_ALEX, Some(&fiona)).await;
    get_ok(&app, FIONA_ALEX, Some(&alex)).await;
}

#[sqlx::test(fixtures("profiles"))]
async fn refuses_bad_requests(db: PgPool) {
    let app = app(db);
    for (uri, status) in [
        (
            "/api/compare/fiona/default/nobody/default",
            StatusCode::NOT_FOUND,
        ),
        (
            "/api/compare/fiona/default/fiona/default",
            StatusCode::BAD_REQUEST,
        ),
        (
            &format!("{FIONA_ALEX}?from=2016-02-01&to=2016-01-01"),
            StatusCode::BAD_REQUEST,
        ),
        (
            &format!("{FIONA_ALEX}?tz=Mars/Olympus"),
            StatusCode::BAD_REQUEST,
        ),
    ] {
        assert_eq!(get(&app, uri, None).await.0, status, "{uri}");
    }
}
