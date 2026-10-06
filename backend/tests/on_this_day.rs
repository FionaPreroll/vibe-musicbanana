//! "On this day" on a profile page.

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

// fixtures/profiles.sql: Fiona heard Björk on 2016-03-01 and 03-02 and Die Ärzte
// on 03-03 and 03-04, all at noon UTC; earlier 2015-06-01 to 06-03 and Tiësto at
// 2015-12-31 23:30 UTC.

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

/// `[date, years_ago, listens]` per year.
fn years(day: &Value) -> Vec<Value> {
    day["years"]
        .as_array()
        .unwrap()
        .iter()
        .map(|y| json!([y["date"], y["years_ago"], y["listens"]]))
        .collect()
}

#[sqlx::test(fixtures("profiles"))]
async fn the_same_date_in_earlier_years(db: PgPool) {
    listen(
        &db,
        &[
            // A year ago: Björk twice, then Die Ärzte with two tracks.
            ("2025-03-03 08:00Z", 2, 3),
            ("2025-03-03 09:00Z", 2, 4),
            ("2025-03-03 10:00Z", 1, 1),
            ("2025-03-03 11:00Z", 1, 2),
            ("2025-03-03 12:00Z", 2, 3),
            // Not that date.
            ("2025-03-02 23:59Z", 3, 5),
            ("2025-03-04 00:00Z", 3, 5),
            // The day itself isn't looked back at.
            ("2026-03-03 07:00Z", 3, 5),
        ],
    )
    .await;

    let day = get_ok(
        &db,
        "/api/profiles/fiona/default/on-this-day?day=2026-03-03",
    )
    .await;
    assert_eq!(day["day"], "2026-03-03");
    // Only years with listens, the latest first.
    assert_eq!(
        years(&day),
        [json!(["2025-03-03", 1, 5]), json!(["2016-03-03", 10, 1])]
    );
    assert_eq!(
        day["years"][0]["artists"],
        json!([
            { "id": 2, "name": "Björk", "listens": 3 },
            { "id": 1, "name": "Die Ärzte", "listens": 2 },
        ])
    );
    assert_eq!(
        day["years"][0]["tracks"],
        json!([
            { "id": 3, "name": "Human Behaviour", "artist": "Björk", "listens": 2 },
            { "id": 2, "name": "Deine Schuld", "artist": "Die Ärzte", "listens": 1 },
            { "id": 4, "name": "Jóga", "artist": "Björk", "listens": 1 },
            { "id": 1, "name": "Unrockbar", "artist": "Die Ärzte", "listens": 1 },
        ])
    );
    assert_eq!(
        day["years"][1]["tracks"],
        json!([{ "id": 1, "name": "Unrockbar", "artist": "Die Ärzte", "listens": 1 }])
    );

    // Nothing before the first listen, and nothing on a date without listens.
    let none = get_ok(
        &db,
        "/api/profiles/fiona/default/on-this-day?day=2026-07-01",
    )
    .await;
    assert_eq!(none["years"], json!([]));
    let alex = get_ok(&db, "/api/profiles/alex/default/on-this-day?day=2017-04-02").await;
    assert_eq!(years(&alex), [json!(["2016-04-02", 1, 1])]);
}

#[sqlx::test(fixtures("profiles"))]
async fn at_most_three_artists_and_five_tracks(db: PgPool) {
    sqlx::raw_sql(
        "INSERT INTO artist (id, name) OVERRIDING SYSTEM VALUE VALUES (4, 'Wir sind Helden');
         INSERT INTO recording (id, title, artist_id) OVERRIDING SYSTEM VALUE VALUES (6, 'Denkmal', 4)",
    )
    .execute(&db)
    .await
    .unwrap();
    let mut listens = Vec::new();
    for (i, at) in ["01", "02", "03", "04", "05", "06", "07", "08"]
        .iter()
        .enumerate()
    {
        let at = format!("2024-03-03 {at}:00Z");
        listens.push((at, [1, 1, 2, 2, 3, 3, 1, 4][i], [1, 2, 3, 4, 5, 1, 1, 6][i]));
    }
    let listens: Vec<_> = listens
        .iter()
        .map(|(at, a, r)| (at.as_str(), *a, *r))
        .collect();
    listen(&db, &listens).await;

    let day = get_ok(
        &db,
        "/api/profiles/fiona/default/on-this-day?day=2026-03-03",
    )
    .await;
    let year = &day["years"][0];
    assert_eq!(year["date"], "2024-03-03");
    assert_eq!(year["listens"], 8);
    let names = |list: &Value| -> Vec<String> {
        let list = list.as_array().unwrap();
        list.iter()
            .map(|e| e["name"].as_str().unwrap().into())
            .collect()
    };
    // Artists count by the listen, tracks under their own artist: Tiësto's
    // second listen is of a Die Ärzte track. Wir sind Helden has the fewest.
    assert_eq!(names(&year["artists"]), ["Die Ärzte", "Björk", "Tiësto"]);
    // Ties go by title, so Jóga is left out.
    assert_eq!(
        names(&year["tracks"]),
        [
            "Unrockbar",
            "Adagio for Strings",
            "Deine Schuld",
            "Denkmal",
            "Human Behaviour"
        ]
    );
    assert_eq!(year["tracks"][0]["listens"], 3);
}

#[sqlx::test(fixtures("profiles"))]
async fn leap_days(db: PgPool) {
    listen(
        &db,
        &[("2016-02-29 12:00Z", 1, 1), ("2019-02-28 12:00Z", 2, 3)],
    )
    .await;

    // 29 February looks back to the 28th in other years.
    let day = get_ok(
        &db,
        "/api/profiles/fiona/default/on-this-day?day=2020-02-29",
    )
    .await;
    assert_eq!(
        years(&day),
        [json!(["2019-02-28", 1, 1]), json!(["2016-02-29", 4, 1])]
    );
}

#[sqlx::test(fixtures("profiles"))]
async fn days_in_the_time_zone(db: PgPool) {
    // Tiësto's listen falls on New Year's Eve in UTC and on New Year's Day in Berlin.
    let utc = get_ok(
        &db,
        "/api/profiles/fiona/default/on-this-day?day=2016-12-31",
    )
    .await;
    assert_eq!(years(&utc), [json!(["2015-12-31", 1, 1])]);
    let berlin = get_ok(
        &db,
        "/api/profiles/fiona/default/on-this-day?day=2016-12-31&tz=Europe/Berlin",
    )
    .await;
    assert_eq!(berlin["years"], json!([]));
    let berlin = get_ok(
        &db,
        "/api/profiles/fiona/default/on-this-day?day=2017-01-01&tz=Europe/Berlin",
    )
    .await;
    assert_eq!(years(&berlin), [json!(["2016-01-01", 1, 1])]);

    // Only the source asked for counts.
    let spotify = get_ok(
        &db,
        "/api/profiles/fiona/default/on-this-day?day=2016-12-31&source=spotify",
    )
    .await;
    assert_eq!(spotify["years"], json!([]));
}

#[sqlx::test(fixtures("profiles"))]
async fn today_by_default(db: PgPool) {
    sqlx::query(
        "INSERT INTO listen (profile_id, listened_at, artist_raw, track_raw, artist_id, recording_id)
         VALUES (1, now() - interval '1 year', 'x', 'y', 1, 1)",
    )
    .execute(&db)
    .await
    .unwrap();
    let today: String = sqlx::query_scalar("SELECT (now() AT TIME ZONE 'UTC')::date::text")
        .fetch_one(&db)
        .await
        .unwrap();

    let day = get_ok(&db, "/api/profiles/fiona/default/on-this-day").await;
    assert_eq!(day["day"], today.as_str());
    assert_eq!(day["years"][0]["years_ago"], 1);
}

#[sqlx::test(fixtures("profiles"))]
async fn refuses_what_it_cannot_show(db: PgPool) {
    for uri in [
        "/api/profiles/fiona/arbeit/on-this-day",
        "/api/profiles/nobody/default/on-this-day",
    ] {
        assert_eq!(get(&db, uri).await.0, StatusCode::NOT_FOUND, "{uri}");
    }
    for uri in [
        "/api/profiles/fiona/default/on-this-day?tz=Mars/Olympus",
        "/api/profiles/fiona/default/on-this-day?day=2016-02-30",
        "/api/profiles/fiona/default/on-this-day?day=yesterday",
    ] {
        assert_eq!(get(&db, uri).await.0, StatusCode::BAD_REQUEST, "{uri}");
    }
}
