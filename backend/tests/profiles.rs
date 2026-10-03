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
        "/api/profiles/fiona/arbeit/top/artists/years",
        "/api/profiles/fiona/arbeit/listens",
        "/api/profiles/fiona/arbeit/artists/3",
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
async fn charts_for_any_period_of_days(db: PgPool) {
    let base = "/api/profiles/fiona/default/top";
    // The first and the last day count in full.
    let period = "from=2016-03-01&to=2016-03-03";
    assert_eq!(
        chart(&get_ok(&db, &format!("{base}/artists?{period}")).await),
        [json!(["Björk", 2]), json!(["Die Ärzte", 1])]
    );
    assert_eq!(
        chart(&get_ok(&db, &format!("{base}/releases?{period}")).await),
        [
            json!(["Debut", "Björk", 1]),
            json!(["Geräusch", "Die Ärzte", 1])
        ]
    );
    assert_eq!(
        chart(&get_ok(&db, &format!("{base}/recordings?{period}")).await),
        [
            json!(["Human Behaviour", "Björk", 1]),
            json!(["Jóga", "Björk", 1]),
            json!(["Unrockbar", "Die Ärzte", 1]),
        ]
    );

    // A period can be open at either end.
    assert_eq!(
        chart(&get_ok(&db, &format!("{base}/artists?from=2016-03-03")).await),
        [json!(["Die Ärzte", 2])]
    );
    assert_eq!(
        chart(&get_ok(&db, &format!("{base}/artists?to=2015-06-02")).await),
        [json!(["Die Ärzte", 2])]
    );

    // The listen at 2015-12-31 23:30 UTC is on New Year's Day in Berlin.
    let new_year = "from=2016-01-01&to=2016-01-01";
    assert_eq!(
        chart(&get_ok(&db, &format!("{base}/artists?{new_year}&tz=Europe/Berlin")).await),
        [json!(["Tiësto", 1])]
    );
    assert_eq!(
        get_ok(&db, &format!("{base}/artists?{new_year}")).await,
        json!([])
    );
}

#[sqlx::test(fixtures("profiles"))]
async fn top_artists_of_each_year(db: PgPool) {
    let base = "/api/profiles/fiona/default/top/artists/years";
    let years = |json: Value| -> Vec<Value> {
        json.as_array()
            .unwrap()
            .iter()
            .map(|y| json!([y["year"], y["listens"], chart(&y["artists"])]))
            .collect()
    };

    let berlin = get_ok(&db, &format!("{base}?tz=Europe/Berlin")).await;
    assert_eq!(
        years(berlin.clone()),
        [
            json!([2015, 3, [["Die Ärzte", 2], ["Björk", 1]]]),
            json!([2016, 5, [["Björk", 2], ["Die Ärzte", 2], ["Tiësto", 1]]]),
        ]
    );
    assert_eq!(berlin[0]["artists"][0]["id"], 1);

    // Only the top of each year, but the year still counts all its listens.
    assert_eq!(
        years(get_ok(&db, &format!("{base}?limit=1")).await),
        [
            json!([2015, 4, [["Die Ärzte", 2]]]),
            json!([2016, 4, [["Björk", 2]]]),
        ]
    );

    let alex = get_ok(&db, "/api/profiles/alex/default/top/artists/years").await;
    assert_eq!(years(alex), [json!([2016, 1, [["Die Ärzte", 1]]])]);
}

#[sqlx::test(fixtures("profiles"))]
async fn bad_parameters_are_rejected(db: PgPool) {
    for uri in [
        "/api/profiles/fiona/default?tz=Mars/Olympus",
        "/api/profiles/fiona/default/top/artists?tz=Mars/Olympus",
        "/api/profiles/fiona/default/top/artists?year=0",
        "/api/profiles/fiona/default/top/artists?year=abc",
        "/api/profiles/fiona/default/top/artists?year=2016&from=2016-01-01",
        "/api/profiles/fiona/default/top/artists?from=yesterday",
        "/api/profiles/fiona/default/top/artists?from=2016-02-30",
        "/api/profiles/fiona/default/top/artists?to=0000-12-31",
        "/api/profiles/fiona/default/top/artists?from=2016-03-04&to=2016-03-01",
        "/api/profiles/fiona/default/top/artists/years?tz=Mars/Olympus",
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
            "artist_id": 1,
            "recording_id": 1,
            "release_id": null,
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

/// `[month, listens]` for every month with listens.
fn months_heard(page: &Value) -> Vec<Value> {
    page["months"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|m| m["listens"] != 0)
        .map(|m| json!([m["month"], m["listens"]]))
        .collect()
}

#[sqlx::test(fixtures("profiles"))]
async fn artist_page(db: PgPool) {
    let page = get_ok(&db, "/api/profiles/fiona/default/artists/1").await;
    assert_eq!(
        page["profile"],
        json!({ "username": "Fiona", "slug": "default", "name": "Default" })
    );
    assert_eq!(page["id"], 1);
    assert_eq!(page["name"], "Die Ärzte");
    assert_eq!(page.get("artist"), None);
    assert_eq!(page["listens"], 4);
    assert_eq!(page["first_listened_at"], "2015-06-01T12:00:00Z");
    assert_eq!(page["last_listened_at"], "2016-03-04T12:00:00Z");

    // Every month of the profile, so the curves of all its pages line up.
    let months = page["months"].as_array().unwrap();
    assert_eq!(months.len(), 10);
    assert_eq!(months[0], json!({ "month": "2015-06", "listens": 2 }));
    assert_eq!(months[9], json!({ "month": "2016-03", "listens": 2 }));
    assert_eq!(
        months_heard(&page),
        [json!(["2015-06", 2]), json!(["2016-03", 2])]
    );
    assert_eq!(page["phases"], json!([]));

    assert_eq!(
        chart(&page["releases"]),
        [json!(["Geräusch", "Die Ärzte", 3])]
    );
    assert_eq!(
        chart(&page["recordings"]),
        [
            json!(["Unrockbar", "Die Ärzte", 3]),
            json!(["Deine Schuld", "Die Ärzte", 1])
        ]
    );
    assert_eq!(page["recordings"][0]["id"], 1);
}

#[sqlx::test(fixtures("profiles"))]
async fn release_and_recording_pages(db: PgPool) {
    let base = "/api/profiles/fiona/default";

    let release = get_ok(&db, &format!("{base}/releases/1")).await;
    assert_eq!(release["name"], "Geräusch");
    assert_eq!(release["artist"], json!({ "id": 1, "name": "Die Ärzte" }));
    assert_eq!(release["listens"], 3);
    assert_eq!(release["last_listened_at"], "2016-03-03T12:00:00Z");
    assert_eq!(
        chart(&release["recordings"]),
        [
            json!(["Unrockbar", "Die Ärzte", 2]),
            json!(["Deine Schuld", "Die Ärzte", 1])
        ]
    );
    assert_eq!(release["releases"], json!([]));

    // Also the listen without an album counts for the track.
    let recording = get_ok(&db, &format!("{base}/recordings/1")).await;
    assert_eq!(recording["name"], "Unrockbar");
    assert_eq!(recording["artist"], json!({ "id": 1, "name": "Die Ärzte" }));
    assert_eq!(recording["listens"], 3);
    assert_eq!(
        months_heard(&recording),
        [json!(["2015-06", 1]), json!(["2016-03", 2])]
    );
    assert_eq!(
        chart(&recording["releases"]),
        [json!(["Geräusch", "Die Ärzte", 2])]
    );
    assert_eq!(recording["recordings"], json!([]));
}

#[sqlx::test(fixtures("profiles"))]
async fn months_follow_the_time_zone(db: PgPool) {
    // The listen at 2015-12-31 23:30 UTC belongs to January in Berlin.
    let base = "/api/profiles/fiona/default/artists/3";
    let utc = get_ok(&db, base).await;
    assert_eq!(months_heard(&utc), [json!(["2015-12", 1])]);
    let berlin = get_ok(&db, &format!("{base}?tz=Europe/Berlin")).await;
    assert_eq!(months_heard(&berlin), [json!(["2016-01", 1])]);
    assert_eq!(berlin["months"].as_array().unwrap().len(), 10);
}

#[sqlx::test(fixtures("profiles"))]
async fn pages_show_phases_of_heavy_listening(db: PgPool) {
    sqlx::query(
        "INSERT INTO listen (profile_id, listened_at, artist_raw, track_raw, artist_id, recording_id)
         SELECT 1, '2016-03-10 12:00:00+00'::timestamptz + n * interval '1 hour',
                'Die Ärzte', 'Unrockbar', 1, 1
           FROM generate_series(1, 5) n",
    )
    .execute(&db)
    .await
    .unwrap();

    let page = get_ok(&db, "/api/profiles/fiona/default/recordings/1").await;
    assert_eq!(page["listens"], 8);
    assert_eq!(
        page["phases"],
        json!([{ "from": "2016-03", "to": "2016-03", "listens": 7 }])
    );
}

#[sqlx::test(fixtures("profiles"))]
async fn merged_entries_show_where_they_went(db: PgPool) {
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO artist (name, merged_into) VALUES ('Die Aerzte', 1) RETURNING id",
    )
    .fetch_one(&db)
    .await
    .unwrap();

    let page = get_ok(&db, &format!("/api/profiles/fiona/default/artists/{id}")).await;
    assert_eq!(page["id"], 1);
    assert_eq!(page["name"], "Die Ärzte");
    assert_eq!(page["listens"], 4);
}

#[sqlx::test(fixtures("profiles"))]
async fn entries_a_profile_never_heard(db: PgPool) {
    // alex only heard Die Ärzte; Björk exists, just not in this profile.
    let page = get_ok(&db, "/api/profiles/alex/default/artists/2").await;
    assert_eq!(page["name"], "Björk");
    assert_eq!(page["listens"], 0);
    assert_eq!(page["first_listened_at"], Value::Null);
    assert_eq!(
        page["months"],
        json!([{ "month": "2016-04", "listens": 0 }])
    );
    assert_eq!(page["phases"], json!([]));
    assert_eq!(page["recordings"], json!([]));

    let base = "/api/profiles/fiona/default";
    for uri in ["artists/99", "releases/99", "recordings/99"] {
        let uri = format!("{base}/{uri}");
        assert_eq!(get(&db, &uri).await.0, StatusCode::NOT_FOUND, "GET {uri}");
    }
    assert_eq!(
        get(&db, &format!("{base}/artists/abc")).await.0,
        StatusCode::BAD_REQUEST
    );
}

#[sqlx::test(fixtures("profiles"))]
async fn a_year_without_listens_is_left_out_of_the_months(db: PgPool) {
    // alex heard something in April 2016. Add April 2017, after eleven quiet
    // months, and May 2018, after twelve.
    sqlx::query(
        "INSERT INTO listen (profile_id, listened_at, artist_raw, track_raw, artist_id, recording_id)
         VALUES (3, '2017-04-02 12:00:00+00', 'Die Ärzte', 'Deine Schuld', 1, 2),
                (3, '2018-05-02 12:00:00+00', 'Die Ärzte', 'Deine Schuld', 1, 2)",
    )
    .execute(&db)
    .await
    .unwrap();

    let page = get_ok(&db, "/api/profiles/alex/default/recordings/2").await;
    let months: Vec<&str> = page["months"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["month"].as_str().unwrap())
        .collect();
    assert_eq!(months.len(), 14);
    assert_eq!(months[..2], ["2016-04", "2016-05"]);
    assert_eq!(months[12..], ["2017-04", "2018-05"]);
    assert_eq!(
        months_heard(&page),
        [
            json!(["2016-04", 1]),
            json!(["2017-04", 1]),
            json!(["2018-05", 1])
        ]
    );
}
