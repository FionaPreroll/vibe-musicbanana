use std::path::Path;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use musicbanana::{AppState, router};
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;

// fixtures/profiles.sql: Fiona has eight public listens in 2015/2016, one more in a
// private profile, and alex has one.

async fn get(db: &PgPool, uri: &str) -> (StatusCode, Value) {
    let app = router(AppState { db: db.clone() }, Path::new("does-not-exist"));
    let res = app
        .oneshot(Request::get(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = res.status();
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json = serde_json::from_slice(&body).unwrap_or(Value::Null);
    (status, json)
}

async fn get_ok(db: &PgPool, uri: &str) -> Value {
    let (status, json) = get(db, uri).await;
    assert_eq!(status, StatusCode::OK, "GET {uri}");
    json
}

/// `[name, listens]` per chart entry, plus the artist for releases and recordings.
fn chart(json: &Value) -> Vec<Value> {
    json.as_array()
        .unwrap()
        .iter()
        .map(|e| match e.get("artist") {
            Some(artist) => json!([e["name"], artist, e["listens"]]),
            None => json!([e["name"], e["listens"]]),
        })
        .collect()
}

#[sqlx::test(fixtures("profiles"))]
async fn lists_public_profiles(db: PgPool) {
    let json = get_ok(&db, "/api/profiles").await;
    assert_eq!(
        json,
        json!([
            { "username": "alex", "slug": "default", "name": "Default", "listens": 1 },
            { "username": "Fiona", "slug": "default", "name": "Default", "listens": 8 },
        ])
    );
}

#[sqlx::test(fixtures("profiles"))]
async fn overview_counts_listens_per_year_in_the_given_time_zone(db: PgPool) {
    let json = get_ok(&db, "/api/profiles/fiona/default?tz=Europe/Berlin").await;
    assert_eq!(
        json,
        json!({
            "username": "Fiona",
            "slug": "default",
            "name": "Default",
            "listens": 8,
            "first_listened_at": "2015-06-01T12:00:00Z",
            "last_listened_at": "2016-03-04T12:00:00Z",
            "years": [{ "year": 2015, "listens": 3 }, { "year": 2016, "listens": 5 }],
        })
    );

    let utc = get_ok(&db, "/api/profiles/fiona/default").await;
    assert_eq!(
        utc["years"],
        json!([{ "year": 2015, "listens": 4 }, { "year": 2016, "listens": 4 }])
    );
}

#[sqlx::test(fixtures("profiles"))]
async fn usernames_and_slugs_are_case_insensitive(db: PgPool) {
    let json = get_ok(&db, "/api/profiles/FIONA/Default").await;
    assert_eq!(json["username"], "Fiona");
}

#[sqlx::test(fixtures("profiles"))]
async fn private_and_unknown_profiles_are_not_found(db: PgPool) {
    for uri in [
        "/api/profiles/fiona/arbeit",
        "/api/profiles/fiona/arbeit/top/artists",
        "/api/profiles/fiona/arbeit/listens",
        "/api/profiles/nobody/default",
        "/api/profiles/fiona/default/top/genres",
    ] {
        assert_eq!(get(&db, uri).await.0, StatusCode::NOT_FOUND, "GET {uri}");
    }
}

#[sqlx::test(fixtures("profiles"))]
async fn all_time_charts(db: PgPool) {
    let base = "/api/profiles/fiona/default/top";
    assert_eq!(
        chart(&get_ok(&db, &format!("{base}/artists")).await),
        [
            json!(["Die Ärzte", 4]),
            json!(["Björk", 3]),
            json!(["Tiësto", 1])
        ]
    );
    // Listens without an album do not count for any release.
    assert_eq!(
        chart(&get_ok(&db, &format!("{base}/releases")).await),
        [
            json!(["Geräusch", "Die Ärzte", 3]),
            json!(["Debut", "Björk", 2])
        ]
    );
    // A recording counts with and without an album; ties are sorted by title.
    assert_eq!(
        chart(&get_ok(&db, &format!("{base}/recordings")).await),
        [
            json!(["Unrockbar", "Die Ärzte", 3]),
            json!(["Human Behaviour", "Björk", 2]),
            json!(["Adagio for Strings", "Tiësto", 1]),
            json!(["Deine Schuld", "Die Ärzte", 1]),
            json!(["Jóga", "Björk", 1]),
        ]
    );
    assert_eq!(
        chart(&get_ok(&db, &format!("{base}/artists?limit=1")).await),
        [json!(["Die Ärzte", 4])]
    );
}

#[sqlx::test(fixtures("profiles"))]
async fn yearly_charts_follow_the_time_zone(db: PgPool) {
    let base = "/api/profiles/fiona/default/top/artists";
    // The listen at 2015-12-31 23:30 UTC belongs to 2016 in Berlin.
    assert_eq!(
        chart(&get_ok(&db, &format!("{base}?year=2016&tz=Europe/Berlin")).await),
        [
            json!(["Björk", 2]),
            json!(["Die Ärzte", 2]),
            json!(["Tiësto", 1])
        ]
    );
    assert_eq!(
        chart(&get_ok(&db, &format!("{base}?year=2016")).await),
        [json!(["Björk", 2]), json!(["Die Ärzte", 2])]
    );
    assert_eq!(
        chart(&get_ok(&db, &format!("{base}?year=2015")).await),
        [
            json!(["Die Ärzte", 2]),
            json!(["Björk", 1]),
            json!(["Tiësto", 1])
        ]
    );
    assert_eq!(get_ok(&db, &format!("{base}?year=2010")).await, json!([]));
}

#[sqlx::test(fixtures("profiles"))]
async fn bad_parameters_are_rejected(db: PgPool) {
    for uri in [
        "/api/profiles/fiona/default?tz=Mars/Olympus",
        "/api/profiles/fiona/default/top/artists?tz=Mars/Olympus",
        "/api/profiles/fiona/default/top/artists?year=0",
        "/api/profiles/fiona/default/top/artists?year=abc",
        "/api/profiles/fiona/default/listens?before=yesterday",
    ] {
        assert_eq!(get(&db, uri).await.0, StatusCode::BAD_REQUEST, "GET {uri}");
    }
}

#[sqlx::test(fixtures("profiles"))]
async fn recent_listens_page_backwards(db: PgPool) {
    let base = "/api/profiles/fiona/default/listens?limit=3";

    let first = get_ok(&db, base).await;
    assert_eq!(
        first["listens"][0],
        json!({
            "listened_at": "2016-03-04T12:00:00Z",
            "artist": "Die Ärzte",
            "track": "Unrockbar",
            "album": null,
        })
    );
    let tracks = |page: &Value| -> Vec<String> {
        page["listens"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| l["track"].as_str().unwrap().to_owned())
            .collect()
    };
    assert_eq!(tracks(&first), ["Unrockbar", "Unrockbar", "Jóga"]);
    assert_eq!(first["next"], "2016-03-02T12:00:00Z");

    // The same instant with an offset; `+` has to be escaped in a query string.
    let second = get_ok(&db, &format!("{base}&before=2016-03-02T13:00:00%2B01:00")).await;
    assert_eq!(
        tracks(&second),
        ["Human Behaviour", "Adagio for Strings", "Human Behaviour"]
    );
    assert_eq!(second["listens"][0]["album"], "Debut");
    assert_eq!(second["next"], "2015-06-03T12:00:00Z");

    let last = get_ok(&db, &format!("{base}&before=2015-06-03T12:00:00Z")).await;
    assert_eq!(tracks(&last), ["Deine Schuld", "Unrockbar"]);
    assert_eq!(last["next"], Value::Null);
}
