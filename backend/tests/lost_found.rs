//! "Lost & found" on a profile page, and dismissing its entries.

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

// fixtures/profiles.sql: Fiona (account 1) heard Die Ärzte, Björk and Tiësto a
// few times in 2015 and 2016 on her default profile; alex (account 2) has a
// public default profile of his own.

const LOST: &str = "/api/profiles/fiona/default/lost-and-found";
const HIDDEN: &str = "/api/me/profiles/default/lost-and-found/hidden";

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

async fn get_ok(app: &Router, uri: &str, cookie: Option<&str>) -> Value {
    let reply = call(app, Method::GET, uri, cookie, None).await;
    assert_eq!(reply.status, StatusCode::OK, "GET {uri}: {}", reply.body);
    reply.body
}

async fn log_in(app: &Router, db: &PgPool, account: i64, login: &str) -> String {
    sqlx::query("UPDATE account SET password_hash = $2 WHERE id = $1")
        .bind(account)
        .bind(auth::hash_password("banana").unwrap())
        .execute(db)
        .await
        .unwrap();
    let reply = call(
        app,
        Method::POST,
        "/api/session",
        None,
        Some(json!({"login": login, "password": "banana"})),
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
    reply.cookie.unwrap().split(';').next().unwrap().to_owned()
}

fn app(db: PgPool) -> Router {
    router(AppState::new(db), Path::new("does-not-exist"))
}

/// Adds `n` listens a day apart from `start` to Fiona's default profile.
async fn listen(db: &PgPool, start: &str, n: i32, artist: i64, recording: i64, client: &str) {
    sqlx::query(
        "INSERT INTO listen (profile_id, listened_at, artist_raw, track_raw, client, artist_id, recording_id)
         SELECT 1, $1::text::timestamptz + make_interval(days => i), 'x', 'y', $2, $3, $4
           FROM generate_series(0, $5 - 1) i",
    )
    .bind(start)
    .bind(client)
    .bind(artist)
    .bind(recording)
    .bind(n)
    .execute(db)
    .await
    .unwrap();
}

/// Björk and Tiësto a lot in 2019 and 2020, Die Ärzte then and still lately,
/// but not "Unrockbar".
async fn history(db: &PgPool) {
    listen(db, "2019-01-01 12:00Z", 25, 2, 3, "Navidrome 0.64.2").await;
    listen(db, "2020-01-01 12:00Z", 12, 3, 5, "").await;
    listen(db, "2019-06-01 12:00Z", 30, 1, 1, "Navidrome 0.64.2").await;
    listen(db, "2026-09-01 12:00Z", 1, 1, 2, "Navidrome 0.64.2").await;
}

/// `[id, listens]` of the artists or tracks.
fn entries(lost: &Value, list: &str) -> Vec<Value> {
    lost[list]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| json!([e["id"], e["listens"]]))
        .collect()
}

#[sqlx::test(fixtures("profiles"))]
async fn heard_a_lot_and_not_for_a_year(db: PgPool) {
    history(&db).await;
    let app = app(db);

    let lost = get_ok(&app, LOST, None).await;
    // A year before the latest listen.
    assert_eq!(lost["since"], "2025-09-01T12:00:00Z");
    // Björk: 25 listens and the 3 in the fixture. Die Ärzte was heard lately,
    // Tiësto only 13 times.
    assert_eq!(
        lost["artists"],
        json!([{
            "id": 2, "name": "Björk", "listens": 28,
            "first_listened_at": "2015-06-03T12:00:00Z",
            "last_listened_at": "2019-01-25T12:00:00Z",
        }])
    );
    // A track can be lost while its artist isn't.
    assert_eq!(lost["tracks"][0]["name"], "Unrockbar");
    assert_eq!(lost["tracks"][0]["artist"], "Die Ärzte");
    assert_eq!(
        entries(&lost, "tracks"),
        [json!([1, 33]), json!([3, 27]), json!([5, 13])]
    );
    // Only the owner learns what they dismissed.
    assert!(lost.get("hidden").is_none());

    // A source narrows what was heard a lot.
    let navidrome = get_ok(&app, &format!("{LOST}?source=Navidrome"), None).await;
    assert_eq!(entries(&navidrome, "artists"), [json!([2, 25])]);
    assert_eq!(
        entries(&navidrome, "tracks"),
        [json!([1, 30]), json!([3, 25])]
    );

    let one = get_ok(&app, &format!("{LOST}?limit=1"), None).await;
    assert_eq!(entries(&one, "tracks"), [json!([1, 33])]);

    // Not enough listens anywhere on alex's profile; nothing at all on an empty one.
    let alex = get_ok(&app, "/api/profiles/alex/default/lost-and-found", None).await;
    assert_eq!(alex["artists"], json!([]));
    assert_eq!(alex["tracks"], json!([]));
}

#[sqlx::test(fixtures("profiles"))]
async fn an_empty_profile_has_nothing_lost(db: PgPool) {
    sqlx::query("DELETE FROM listen WHERE profile_id = 1")
        .execute(&db)
        .await
        .unwrap();
    let lost = get_ok(&app(db), LOST, None).await;
    assert_eq!(lost, json!({ "since": null, "artists": [], "tracks": [] }));
}

#[sqlx::test(fixtures("profiles"))]
async fn the_owner_dismisses_and_shows_again(db: PgPool) {
    history(&db).await;
    let app = app(db.clone());
    let fiona = log_in(&app, &db, 1, "fiona").await;
    let fiona = Some(fiona.as_str());

    assert_eq!(get_ok(&app, LOST, fiona).await["hidden"], 0);

    // A dismissed artist takes its tracks along.
    let reply = call(
        &app,
        Method::POST,
        HIDDEN,
        fiona,
        Some(json!({"kind": "artist", "id": 2})),
    )
    .await;
    assert_eq!(reply.status, StatusCode::NO_CONTENT, "{}", reply.body);
    let lost = get_ok(&app, LOST, fiona).await;
    assert_eq!(lost["artists"], json!([]));
    assert_eq!(entries(&lost, "tracks"), [json!([1, 33]), json!([5, 13])]);
    assert_eq!(lost["hidden"], 1);
    // For everybody else as well.
    assert_eq!(get_ok(&app, LOST, None).await["artists"], json!([]));

    // Again is fine.
    let reply = call(
        &app,
        Method::POST,
        HIDDEN,
        fiona,
        Some(json!({"kind": "artist", "id": 2})),
    )
    .await;
    assert_eq!(reply.status, StatusCode::NO_CONTENT);

    let reply = call(
        &app,
        Method::POST,
        HIDDEN,
        fiona,
        Some(json!({"kind": "recording", "id": 5})),
    )
    .await;
    assert_eq!(reply.status, StatusCode::NO_CONTENT);
    let lost = get_ok(&app, LOST, fiona).await;
    assert_eq!(entries(&lost, "tracks"), [json!([1, 33])]);
    assert_eq!(lost["hidden"], 2);

    let hidden = get_ok(&app, HIDDEN, fiona).await;
    let hidden: Vec<Value> = hidden
        .as_array()
        .unwrap()
        .iter()
        .map(|h| json!([h["kind"], h["id"], h["name"], h["artist"]]))
        .collect();
    assert_eq!(
        hidden,
        [
            json!(["recording", 5, "Adagio for Strings", "Tiësto"]),
            json!(["artist", 2, "Björk", null]),
        ]
    );

    // Shown again.
    let reply = call(
        &app,
        Method::DELETE,
        &format!("{HIDDEN}/artist/2"),
        fiona,
        None,
    )
    .await;
    assert_eq!(reply.status, StatusCode::NO_CONTENT);
    let lost = get_ok(&app, LOST, fiona).await;
    assert_eq!(entries(&lost, "artists"), [json!([2, 28])]);
    assert_eq!(entries(&lost, "tracks"), [json!([1, 33]), json!([3, 27])]);
    assert_eq!(lost["hidden"], 1);

    // Unknown kinds and entries.
    let reply = call(
        &app,
        Method::POST,
        HIDDEN,
        fiona,
        Some(json!({"kind": "release", "id": 1})),
    )
    .await;
    assert_eq!(reply.status, StatusCode::BAD_REQUEST);
    let reply = call(
        &app,
        Method::POST,
        HIDDEN,
        fiona,
        Some(json!({"kind": "artist", "id": 99})),
    )
    .await;
    assert_eq!(reply.status, StatusCode::NOT_FOUND);
    let reply = call(
        &app,
        Method::DELETE,
        &format!("{HIDDEN}/release/1"),
        fiona,
        None,
    )
    .await;
    assert_eq!(reply.status, StatusCode::BAD_REQUEST);
}

#[sqlx::test(fixtures("profiles"))]
async fn dismissals_belong_to_one_profile(db: PgPool) {
    history(&db).await;
    let app = app(db.clone());

    let reply = call(
        &app,
        Method::POST,
        HIDDEN,
        None,
        Some(json!({"kind": "artist", "id": 2})),
    )
    .await;
    assert_eq!(reply.status, StatusCode::UNAUTHORIZED);

    // alex dismisses Björk on his own profile, which leaves Fiona's alone.
    let alex = log_in(&app, &db, 2, "alex").await;
    let reply = call(
        &app,
        Method::POST,
        HIDDEN,
        Some(&alex),
        Some(json!({"kind": "artist", "id": 2})),
    )
    .await;
    assert_eq!(reply.status, StatusCode::NO_CONTENT);
    assert_eq!(
        entries(&get_ok(&app, LOST, None).await, "artists"),
        [json!([2, 28])]
    );
    // He has no profile "arbeit".
    let reply = call(
        &app,
        Method::POST,
        "/api/me/profiles/arbeit/lost-and-found/hidden",
        Some(&alex),
        Some(json!({"kind": "artist", "id": 2})),
    )
    .await;
    assert_eq!(reply.status, StatusCode::NOT_FOUND);
}

#[sqlx::test(fixtures("profiles"))]
async fn a_dismissal_follows_a_merge(db: PgPool) {
    history(&db).await;
    // "Bjork" was dismissed, then merged into Björk.
    sqlx::query(
        "INSERT INTO artist (id, name, merged_into) OVERRIDING SYSTEM VALUE VALUES (4, 'Bjork', 2)",
    )
    .execute(&db)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO lost_found_dismissal (profile_id, kind, entity_id) VALUES (1, 'artist', 4)",
    )
    .execute(&db)
    .await
    .unwrap();
    let lost = get_ok(&app(db), LOST, None).await;
    assert_eq!(lost["artists"], json!([]));
    assert_eq!(entries(&lost, "tracks"), [json!([1, 33]), json!([5, 13])]);
}
