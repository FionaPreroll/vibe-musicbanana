//! The week at a glance on top of a profile page.

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

// fixtures/profiles.sql: in the week of Monday 2016-02-29 Fiona heard Björk on
// Tuesday and Wednesday and Die Ärzte on Thursday and Friday, all at noon UTC.
// Before that: 2015-06-01 to 06-03 and Tiësto at 2015-12-31 23:30 UTC.

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

/// Adds listens to Fiona's default profile: `(time, artist id, recording id)`.
async fn listen(db: &PgPool, listens: &[(&str, i64, i64)]) {
    for (at, artist, recording) in listens {
        sqlx::query(
            "INSERT INTO listen (profile_id, listened_at, artist_raw, track_raw, artist_id, recording_id)
             VALUES (1, $1::text::timestamptz, 'x', 'y', $2, $3)",
        )
        .bind(at)
        .bind(artist)
        .bind(recording)
        .execute(db)
        .await
        .unwrap();
    }
}

/// `[listens, last_week]` per day from Monday to Sunday.
fn days(week: &Value) -> Vec<Value> {
    week["days"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| json!([d["listens"], d["last_week"]]))
        .collect()
}

#[sqlx::test(fixtures("profiles"))]
async fn a_week_against_the_week_before(db: PgPool) {
    listen(
        &db,
        &[
            ("2016-02-23 12:00Z", 3, 5),
            ("2016-02-24 12:00Z", 2, 3),
            ("2016-02-26 12:00Z", 2, 4),
            ("2016-03-02 18:00Z", 1, 2),
        ],
    )
    .await;

    // Seen from Thursday: Friday's listen is still to come.
    let week = get_ok(&db, "/api/profiles/fiona/default/week?day=2016-03-03").await;
    assert_eq!(
        [&week["from"], &week["to"], &week["day"]],
        ["2016-02-29", "2016-03-06", "2016-03-03"]
    );
    assert_eq!(
        days(&week),
        [
            json!([0, 0]),
            json!([1, 1]),
            json!([2, 1]),
            json!([1, 0]),
            json!([0, 1]),
            json!([0, 0]),
            json!([0, 0])
        ]
    );
    // Up to Thursday the week before had two listens, all of it three.
    assert_eq!(
        [
            &week["listens"],
            &week["last_week_so_far"],
            &week["last_week"]
        ],
        [4, 2, 3]
    );
    assert_eq!(
        week["artists"],
        json!([
            { "id": 2, "name": "Björk", "listens": 2, "last_week": 2, "new": false },
            { "id": 1, "name": "Die Ärzte", "listens": 2, "last_week": 0, "new": false },
        ])
    );
    assert_eq!(week["new_artists"], 0);
    // Three days in a row now, as long as the three days in June 2015; the
    // latest of the two counts as the longest.
    assert_eq!(
        week["streak"],
        json!({
            "current": 3,
            "current_from": "2016-03-01",
            "longest": 3,
            "longest_from": "2016-03-01",
            "longest_to": "2016-03-03",
        })
    );

    // Any day of the week shows the same week; Sunday shows all of it.
    let sunday = get_ok(&db, "/api/profiles/fiona/default/week?day=2016-03-06").await;
    assert_eq!(sunday["from"], "2016-02-29");
    assert_eq!([&sunday["listens"], &sunday["last_week_so_far"]], [5, 3]);
    // Friday was the last day with listens, so by Sunday the streak is over.
    assert_eq!(sunday["streak"]["current"], 0);
    assert_eq!(sunday["streak"]["current_from"], Value::Null);
    assert_eq!(sunday["streak"]["longest"], 4);
}

#[sqlx::test(fixtures("profiles"))]
async fn days_and_new_artists_in_the_time_zone(db: PgPool) {
    // Tiësto's first listen falls on Thursday in UTC and on New Year's Day in Berlin.
    let utc = get_ok(&db, "/api/profiles/fiona/default/week?day=2016-01-01").await;
    let berlin = get_ok(
        &db,
        "/api/profiles/fiona/default/week?day=2016-01-01&tz=Europe/Berlin",
    )
    .await;
    assert_eq!(utc["from"], "2015-12-28");
    assert_eq!(berlin["from"], "2015-12-28");
    assert_eq!(utc["days"][3]["listens"], 1);
    assert_eq!(berlin["days"][3]["listens"], 0);
    assert_eq!(berlin["days"][4]["listens"], 1);
    for week in [&utc, &berlin] {
        assert_eq!(
            week["artists"],
            json!([{ "id": 3, "name": "Tiësto", "listens": 1, "last_week": 0, "new": true }])
        );
        assert_eq!(week["new_artists"], 1);
        assert_eq!(week["streak"]["current"], 1);
    }
    // A streak still runs while today has no listens yet.
    assert_eq!(utc["streak"]["current_from"], "2015-12-31");
    assert_eq!(berlin["streak"]["current_from"], "2016-01-01");

    // Only the source asked for counts.
    let spotify = get_ok(
        &db,
        "/api/profiles/fiona/default/week?day=2016-01-01&source=spotify",
    )
    .await;
    assert_eq!(spotify["listens"], 0);
    assert_eq!(spotify["artists"], json!([]));
    assert_eq!(spotify["streak"]["longest"], 0);
}

#[sqlx::test(fixtures("profiles"))]
async fn this_week_counts_up_to_now(db: PgPool) {
    sqlx::query(
        "INSERT INTO listen (profile_id, listened_at, artist_raw, track_raw, artist_id, recording_id)
         VALUES (1, now() - interval '1 second', 'x', 'y', 1, 1),
                (1, now() + interval '1 hour', 'x', 'y', 2, 3)",
    )
    .execute(&db)
    .await
    .unwrap();
    let today: String = sqlx::query_scalar("SELECT (now() AT TIME ZONE 'UTC')::date::text")
        .fetch_one(&db)
        .await
        .unwrap();

    let week = get_ok(&db, "/api/profiles/fiona/default/week").await;
    assert_eq!(week["day"], today.as_str());
    // A listen from the future (a player with a wrong clock) waits until its time.
    assert_eq!(week["listens"], 1);
    assert_eq!(week["streak"]["current"], 1);
    assert_eq!(week["streak"]["current_from"], today.as_str());
}

#[sqlx::test(fixtures("profiles"))]
async fn weeks_from_sunday_or_saturday(db: PgPool) {
    // Seen from Thursday 2016-03-03, a week from Sunday began on 2016-02-28;
    // Friday's listen is still to come.
    let week = get_ok(
        &db,
        "/api/profiles/fiona/default/week?day=2016-03-03&week_start=7",
    )
    .await;
    assert_eq!([&week["from"], &week["to"]], ["2016-02-28", "2016-03-05"]);
    let dates: Vec<_> = week["days"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| &d["date"])
        .collect();
    assert_eq!(dates.first().unwrap().as_str(), Some("2016-02-28"));
    assert_eq!(dates.last().unwrap().as_str(), Some("2016-03-05"));
    assert_eq!(
        days(&week),
        [
            json!([0, 0]),
            json!([0, 0]),
            json!([1, 0]),
            json!([1, 0]),
            json!([1, 0]),
            json!([0, 0]),
            json!([0, 0])
        ]
    );

    // A Saturday is the first day of its own week.
    let week = get_ok(
        &db,
        "/api/profiles/fiona/default/week?day=2016-03-05&week_start=6",
    )
    .await;
    assert_eq!([&week["from"], &week["to"]], ["2016-03-05", "2016-03-11"]);
    // Monday is the default.
    let week = get_ok(&db, "/api/profiles/fiona/default/week?day=2016-03-06").await;
    assert_eq!([&week["from"], &week["to"]], ["2016-02-29", "2016-03-06"]);
}

#[sqlx::test(fixtures("profiles"))]
async fn refuses_what_it_cannot_show(db: PgPool) {
    for uri in [
        "/api/profiles/fiona/arbeit/week",
        "/api/profiles/nobody/default/week",
    ] {
        assert_eq!(get(&db, uri).await.0, StatusCode::NOT_FOUND, "{uri}");
    }
    for uri in [
        "/api/profiles/fiona/default/week?tz=Mars/Olympus",
        "/api/profiles/fiona/default/week?day=2016-02-30",
        "/api/profiles/fiona/default/week?day=yesterday",
        "/api/profiles/fiona/default/week?week_start=0",
        "/api/profiles/fiona/default/week?week_start=8",
    ] {
        assert_eq!(get(&db, uri).await.0, StatusCode::BAD_REQUEST, "{uri}");
    }
}
