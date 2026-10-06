//! The year in review of a profile.

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

// fixtures/profiles.sql: Fiona's default profile heard Die Ärzte (1) and Björk (2)
// from 2015-06-01 to 06-03, Tiësto (3) at 2015-12-31 23:30 UTC (2016 in Berlin),
// Björk on 2016-03-01 and 03-02 and Die Ärzte on 03-03 and 03-04, all at noon UTC.

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

/// Adds listens to Fiona's default profile: `(time, artist id, recording id, duration in s)`.
async fn listen(db: &PgPool, listens: &[(&str, i64, i64, Option<i32>)]) {
    for (at, artist, recording, seconds) in listens {
        sqlx::query(
            "INSERT INTO listen (profile_id, listened_at, artist_raw, track_raw, artist_id,
                                 recording_id, duration_ms)
             VALUES (1, $1::text::timestamptz, 'x', 'y', $2, $3, $4 * 1000)",
        )
        .bind(at)
        .bind(artist)
        .bind(recording)
        .bind(seconds)
        .execute(db)
        .await
        .unwrap();
    }
}

/// `[month, listens, last_year]` of the months with any.
fn months(review: &Value) -> Vec<Value> {
    review["months"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|m| m["listens"] != 0 || m["last_year"] != 0)
        .map(|m| json!([m["month"], m["listens"], m["last_year"]]))
        .collect()
}

#[sqlx::test(fixtures("profiles"))]
async fn a_year_against_the_one_before(db: PgPool) {
    let review = get_ok(&db, "/api/profiles/fiona/default/year/2016").await;
    assert_eq!(review["year"], 2016);
    assert_eq!(review["complete"], true);
    assert_eq!(
        [
            &review["listens"],
            &review["last_year_so_far"],
            &review["last_year"],
            &review["artists"],
            &review["recordings"],
            &review["days"],
        ],
        [4, 4, 4, 2, 3, 4]
    );
    assert_eq!(
        months(&review),
        [json!([3, 4, 0]), json!([6, 0, 3]), json!([12, 0, 1])]
    );
    assert_eq!(
        review["top_day"],
        json!({ "date": "2016-03-01", "listens": 1 })
    );

    // Both artists were heard in 2015 already; Björk went from one listen to two
    // and Die Ärzte stayed at two.
    assert_eq!(review["new_artists"], 0);
    assert_eq!(review["discoveries"], json!([]));
    assert_eq!(
        review["risers"],
        json!([{ "id": 2, "name": "Björk", "listens": 2, "last_year": 1, "rank": 1, "last_rank": 2 }])
    );

    // Each listen is a session of its own; without lengths they all last no time,
    // so the first one wins.
    assert_eq!(
        review["longest_session"],
        json!({
            "started_at": "2016-03-01T12:00:00Z",
            "ended_at": "2016-03-01T12:00:00Z",
            "listens": 1,
            "artists": [{ "id": 2, "name": "Björk", "listens": 1 }]
        })
    );
}

#[sqlx::test(fixtures("profiles"))]
async fn the_first_year_discovers_everything(db: PgPool) {
    let review = get_ok(&db, "/api/profiles/fiona/default/year/2015").await;
    assert_eq!([&review["listens"], &review["last_year"]], [4, 0]);
    assert_eq!(review["new_artists"], 3);
    assert_eq!(
        review["discoveries"],
        json!([
            { "id": 1, "name": "Die Ärzte", "listens": 2, "first_listened_at": "2015-06-01T12:00:00Z" },
            { "id": 2, "name": "Björk", "listens": 1, "first_listened_at": "2015-06-03T12:00:00Z" },
            { "id": 3, "name": "Tiësto", "listens": 1, "first_listened_at": "2015-12-31T23:30:00Z" },
        ])
    );
    assert_eq!(review["risers"], json!([]));
}

#[sqlx::test(fixtures("profiles"))]
async fn years_start_at_midnight_in_the_time_zone(db: PgPool) {
    // In Berlin, Tiësto on New Year's Eve 23:30 UTC is a 2016 listen: new then,
    // since nobody heard him in 2015 there.
    let review = get_ok(
        &db,
        "/api/profiles/fiona/default/year/2016?tz=Europe/Berlin",
    )
    .await;
    assert_eq!([&review["listens"], &review["last_year"]], [5, 3]);
    assert_eq!(review["new_artists"], 1);
    assert_eq!(review["discoveries"][0]["name"], "Tiësto");
    assert_eq!(months(&review)[0], json!([1, 1, 0]));
}

#[sqlx::test(fixtures("profiles"))]
async fn the_longest_session_counts_pauses_from_the_end_of_a_track(db: PgPool) {
    listen(
        &db,
        &[
            // 20:00 to 21:20: a 50-minute track, then 29 minutes later the next
            // one; the pause from the end of the first one is short enough.
            ("2016-07-01 20:00:00+00", 2, 3, Some(3000)),
            ("2016-07-01 21:19:00+00", 2, 4, Some(60)),
            ("2016-07-01 21:25:00+00", 1, 1, None),
            // More than 30 minutes after a listen of unknown length: a new session.
            ("2016-07-01 21:56:00+00", 1, 2, Some(7200)),
            // Ten listens in a row, shorter than the session above.
            ("2016-08-01 10:00:00+00", 1, 1, Some(60)),
            ("2016-08-01 10:01:00+00", 1, 1, Some(60)),
            ("2016-08-01 10:02:00+00", 1, 1, Some(60)),
        ],
    )
    .await;
    let review = get_ok(&db, "/api/profiles/fiona/default/year/2016").await;
    assert_eq!(
        review["longest_session"],
        json!({
            "started_at": "2016-07-01T21:56:00Z",
            "ended_at": "2016-07-01T23:56:00Z",
            "listens": 1,
            "artists": [{ "id": 1, "name": "Die Ärzte", "listens": 1 }]
        })
    );

    // Without the long track the evening session is the longest.
    sqlx::query("DELETE FROM listen WHERE listened_at = '2016-07-01 21:56:00+00'")
        .execute(&db)
        .await
        .unwrap();
    let review = get_ok(&db, "/api/profiles/fiona/default/year/2016").await;
    assert_eq!(
        review["longest_session"],
        json!({
            "started_at": "2016-07-01T20:00:00Z",
            "ended_at": "2016-07-01T21:25:00Z",
            "listens": 3,
            "artists": [
                { "id": 2, "name": "Björk", "listens": 2 },
                { "id": 1, "name": "Die Ärzte", "listens": 1 }
            ]
        })
    );
}

#[sqlx::test(fixtures("profiles"))]
async fn a_running_year_is_compared_up_to_the_same_day(db: PgPool) {
    let year: i32 = sqlx::query_scalar("SELECT date_part('year', now())::int")
        .fetch_one(&db)
        .await
        .unwrap();
    // A listen in the year before an hour before now a year ago, and one a day
    // later, which this year hasn't reached yet.
    sqlx::query(
        "INSERT INTO listen (profile_id, listened_at, artist_raw, track_raw, artist_id, recording_id)
         SELECT 1, now() - interval '1 year' - interval '1 hour', 'x', 'y', 2, 3
          UNION ALL
         SELECT 1, now() - interval '1 year' + interval '1 day', 'x', 'y', 2, 3
          UNION ALL
         SELECT 1, now() - interval '1 minute', 'x', 'y', 1, 1",
    )
    .execute(&db)
    .await
    .unwrap();
    let review = get_ok(&db, &format!("/api/profiles/fiona/default/year/{year}")).await;
    assert_eq!(review["complete"], false);
    // Except right after New Year, when "an hour before now a year ago" is two years ago.
    if review["last_year"] == 2 {
        assert_eq!(
            [
                &review["listens"],
                &review["last_year_so_far"],
                &review["last_year"]
            ],
            [1, 1, 2]
        );
    }

    let next = get_ok(
        &db,
        &format!("/api/profiles/fiona/default/year/{}", year + 1),
    )
    .await;
    assert_eq!([&next["listens"], &next["last_year_so_far"]], [0, 0]);
    assert_eq!(next["longest_session"], Value::Null);
    assert_eq!(next["top_day"], Value::Null);
}

#[sqlx::test(fixtures("profiles"))]
async fn the_source_and_the_visibility_count(db: PgPool) {
    sqlx::query("UPDATE listen SET client = 'Navidrome 0.58.0' WHERE listened_at >= '2016-03-03'")
        .execute(&db)
        .await
        .unwrap();
    let review = get_ok(
        &db,
        "/api/profiles/fiona/default/year/2016?source=Navidrome",
    )
    .await;
    assert_eq!([&review["listens"], &review["last_year"]], [2, 0]);
    // Nobody heard Die Ärzte in Navidrome before.
    assert_eq!(review["new_artists"], 1);

    let (status, _) = get(&db, "/api/profiles/fiona/arbeit/year/2016").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = get(&db, "/api/profiles/fiona/default/year/0").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = get(&db, "/api/profiles/fiona/default/year/1").await;
    assert_eq!(status, StatusCode::OK);
}
