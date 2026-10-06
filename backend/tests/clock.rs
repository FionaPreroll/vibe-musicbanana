//! The listening clock: listens by weekday and hour on a profile page.

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

// fixtures/profiles.sql: Fiona heard something at noon UTC on Monday to Wednesday
// 2015-06-01 to 06-03 and Tuesday to Friday 2016-03-01 to 03-04, and Tiësto on
// Thursday 2015-12-31 at 23:30 UTC, already Friday 2016-01-01 in Berlin.

async fn get(db: &PgPool, uri: &str) -> (StatusCode, Value) {
    let app = router(AppState::new(db.clone()), Path::new("does-not-exist"));
    let res = app
        .oneshot(Request::get(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = res.status();
    let body = res.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&body).unwrap_or(Value::Null))
}

async fn get_ok(db: &PgPool, uri: &str) -> Value {
    let (status, json) = get(db, uri).await;
    assert_eq!(status, StatusCode::OK, "GET {uri}");
    json
}

/// The cells with listens as `[weekday (0 = Monday), hour, listens]`.
fn cells(clock: &Value) -> Vec<Value> {
    let weekdays = clock["weekdays"].as_array().unwrap();
    assert_eq!(weekdays.len(), 7);
    let mut cells = Vec::new();
    for (day, hours) in weekdays.iter().enumerate() {
        let hours = hours.as_array().unwrap();
        assert_eq!(hours.len(), 24);
        for (hour, listens) in hours.iter().enumerate() {
            if listens != 0 {
                cells.push(json!([day, hour, listens]));
            }
        }
    }
    cells
}

#[sqlx::test(fixtures("profiles"))]
async fn all_time_in_utc(db: PgPool) {
    let clock = get_ok(&db, "/api/profiles/fiona/default/clock").await;
    assert_eq!(clock["listens"], 8);
    assert_eq!(
        cells(&clock),
        [
            json!([0, 12, 1]),
            json!([1, 12, 2]),
            json!([2, 12, 2]),
            json!([3, 12, 1]),
            json!([3, 23, 1]),
            json!([4, 12, 1]),
        ]
    );
}

#[sqlx::test(fixtures("profiles"))]
async fn local_hours_follow_summer_time(db: PgPool) {
    // Noon UTC is 14:00 in a Berlin summer and 13:00 in its winter.
    let clock = get_ok(&db, "/api/profiles/fiona/default/clock?tz=Europe/Berlin").await;
    assert_eq!(
        cells(&clock),
        [
            json!([0, 14, 1]),
            json!([1, 13, 1]),
            json!([1, 14, 1]),
            json!([2, 13, 1]),
            json!([2, 14, 1]),
            json!([3, 13, 1]),
            json!([4, 0, 1]),
            json!([4, 13, 1]),
        ]
    );
}

#[sqlx::test(fixtures("profiles"))]
async fn a_period_and_a_source(db: PgPool) {
    // In Berlin, Tiësto already belongs to 2016.
    let clock = get_ok(
        &db,
        "/api/profiles/fiona/default/clock?year=2016&tz=Europe/Berlin",
    )
    .await;
    assert_eq!(clock["listens"], 5);
    assert_eq!(cells(&clock)[0], json!([1, 13, 1]));

    let clock = get_ok(
        &db,
        "/api/profiles/fiona/default/clock?from=2016-03-02&to=2016-03-03",
    )
    .await;
    assert_eq!(cells(&clock), [json!([2, 12, 1]), json!([3, 12, 1])]);

    let clock = get_ok(&db, "/api/profiles/fiona/default/clock?source=navidrome").await;
    assert_eq!(clock["listens"], 0);
    assert_eq!(cells(&clock), Vec::<Value>::new());
}

#[sqlx::test(fixtures("profiles"))]
async fn refuses_what_the_charts_refuse(db: PgPool) {
    let (status, _) = get(&db, "/api/profiles/fiona/arbeit/clock").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = get(&db, "/api/profiles/fiona/default/clock?tz=Mars/Olympus").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = get(
        &db,
        "/api/profiles/fiona/default/clock?year=2016&from=2016-01-01",
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
