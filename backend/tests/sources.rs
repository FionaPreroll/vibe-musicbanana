//! Where listens came from, and the charts and pages of one source.

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

// fixtures/profiles.sql: Fiona's default profile has four listens in 2015 and four
// in 2016. Here the 2015 ones came from the old import and the 2016 ones from
// Navidrome.

async fn get(db: &PgPool, uri: &str) -> Value {
    let app = router(AppState::new(db.clone()), Path::new("does-not-exist"));
    let res = app
        .oneshot(Request::get(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK, "GET {uri}");
    let body = res.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&body).unwrap()
}

async fn with_sources(db: &PgPool) {
    sqlx::query(
        "UPDATE listen SET client = CASE WHEN listened_at < '2016-01-01'
                                         THEN 'import:php-2016'
                                         ELSE 'Navidrome 0.64.2 (1a2b3c4)' END
          WHERE profile_id = 1",
    )
    .execute(db)
    .await
    .unwrap();
}

const FIONA: &str = "/api/profiles/fiona/default";

#[sqlx::test(fixtures("profiles"))]
async fn lists_the_sources_without_versions(db: PgPool) {
    with_sources(&db).await;
    // One more listen from Navidrome, in another version.
    sqlx::query(
        "INSERT INTO listen (profile_id, listened_at, artist_raw, track_raw, artist_id, recording_id, client)
         VALUES (1, '2016-03-05 12:00:00+00', 'Björk', 'Jóga', 2, 4, 'Navidrome 0.65.0')",
    )
    .execute(&db)
    .await
    .unwrap();
    assert_eq!(
        get(&db, &format!("{FIONA}/sources")).await,
        json!([
            {
                "source": "Navidrome", "listens": 5,
                "first_listened_at": "2016-03-01T12:00:00Z", "last_listened_at": "2016-03-05T12:00:00Z",
            },
            {
                "source": "import:php-2016", "listens": 4,
                "first_listened_at": "2015-06-01T12:00:00Z", "last_listened_at": "2015-12-31T23:30:00Z",
            },
        ])
    );
    // Listens that named no client.
    assert_eq!(
        get(&db, "/api/profiles/alex/default/sources").await,
        json!([{
            "source": "", "listens": 1,
            "first_listened_at": "2016-04-02T12:00:00Z", "last_listened_at": "2016-04-02T12:00:00Z",
        }])
    );
}

#[sqlx::test(fixtures("profiles"))]
async fn every_page_counts_one_source(db: PgPool) {
    with_sources(&db).await;
    let navidrome = "source=Navidrome";

    let overview = get(&db, &format!("{FIONA}?{navidrome}")).await;
    assert_eq!(overview["listens"], 4);
    assert_eq!(overview["years"], json!([{"year": 2016, "listens": 4}]));
    assert_eq!(overview["first_listened_at"], "2016-03-01T12:00:00Z");

    let names = |json: Value| -> Vec<(String, i64)> {
        json.as_array()
            .unwrap()
            .iter()
            .map(|e| {
                (
                    e["name"].as_str().unwrap().to_owned(),
                    e["listens"].as_i64().unwrap(),
                )
            })
            .collect()
    };
    assert_eq!(
        names(get(&db, &format!("{FIONA}/top/artists?source=import:php-2016")).await),
        [
            ("Die Ärzte".to_owned(), 2),
            ("Björk".to_owned(), 1),
            ("Tiësto".to_owned(), 1)
        ]
    );
    assert_eq!(
        names(get(&db, &format!("{FIONA}/top/recordings?{navidrome}")).await),
        [
            ("Unrockbar".to_owned(), 2),
            ("Human Behaviour".to_owned(), 1),
            ("Jóga".to_owned(), 1)
        ]
    );

    let years = get(&db, &format!("{FIONA}/top/artists/years?{navidrome}")).await;
    assert_eq!(
        years
            .as_array()
            .unwrap()
            .iter()
            .map(|y| y["year"].clone())
            .collect::<Vec<_>>(),
        [json!(2016)]
    );

    let listens = get(&db, &format!("{FIONA}/listens?{navidrome}")).await;
    assert_eq!(listens["listens"].as_array().unwrap().len(), 4);

    let artist = get(&db, &format!("{FIONA}/artists/1?{navidrome}")).await;
    assert_eq!(artist["listens"], 2);
    assert_eq!(artist["first_listened_at"], "2016-03-03T12:00:00Z");
    assert_eq!(
        names(artist["recordings"].clone()),
        [("Unrockbar".to_owned(), 2)]
    );
    assert_eq!(
        names(artist["releases"].clone()),
        [("Geräusch".to_owned(), 1)]
    );
    // The months still run over the whole profile, so the curves line up.
    let months = artist["months"].as_array().unwrap();
    assert_eq!(months[0]["month"], "2015-06");
    assert_eq!(
        months
            .iter()
            .map(|m| m["listens"].as_i64().unwrap())
            .sum::<i64>(),
        2
    );

    let found = get(&db, &format!("{FIONA}/search?q=ärzte&{navidrome}")).await;
    assert_eq!(
        names(found["artists"].clone()),
        [("Die Ärzte".to_owned(), 2)]
    );
}
