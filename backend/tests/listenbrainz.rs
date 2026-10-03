use std::path::Path;

use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use http_body_util::BodyExt;
use musicbanana::{AppState, router, tokens};
use serde_json::{Value, json};
use sqlx::PgPool;
use time::OffsetDateTime;
use tower::ServiceExt;

// fixtures/profiles.sql: Fiona's default profile has eight listens of Die Ärzte,
// Björk and Tiësto in 2015/2016, and every catalog entry has its alias.

const SUBMIT: &str = "/api/listenbrainz/1/submit-listens";
const VALIDATE: &str = "/api/listenbrainz/1/validate-token";

async fn send(db: &PgPool, request: Request<Body>) -> (StatusCode, Value) {
    let app = router(AppState { db: db.clone() }, Path::new("does-not-exist"));
    let res = app.oneshot(request).await.unwrap();
    let status = res.status();
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json = serde_json::from_slice(&body).unwrap_or(Value::Null);
    (status, json)
}

/// POST /submit-listens the way Navidrome sends it.
async fn submit(db: &PgPool, token: &str, body: &Value) -> (StatusCode, Value) {
    let request = Request::post(SUBMIT)
        .header(header::AUTHORIZATION, format!("Token {token}"))
        .header(header::CONTENT_TYPE, "application/json; charset=UTF-8")
        .body(Body::from(body.to_string()))
        .unwrap();
    send(db, request).await
}

async fn submit_ok(db: &PgPool, token: &str, body: &Value) {
    assert_eq!(
        submit(db, token, body).await,
        (StatusCode::OK, json!({ "status": "ok" })),
        "{body}"
    );
}

async fn get(db: &PgPool, uri: &str) -> (StatusCode, Value) {
    send(db, Request::get(uri).body(Body::empty()).unwrap()).await
}

async fn fiona_token(db: &PgPool) -> String {
    tokens::create(db, "fiona", "default", "Navidrome")
        .await
        .unwrap()
        .token
}

fn now() -> i64 {
    OffsetDateTime::now_utc().unix_timestamp()
}

fn listen(listened_at: i64, artist: &str, track: &str, album: Option<&str>) -> Value {
    let mut meta = json!({ "artist_name": artist, "track_name": track });
    if let Some(album) = album {
        meta["release_name"] = json!(album);
    }
    json!({ "listened_at": listened_at, "track_metadata": meta })
}

fn single(listen: Value) -> Value {
    json!({ "listen_type": "single", "payload": [listen] })
}

fn import(listens: Vec<Value>) -> Value {
    json!({ "listen_type": "import", "payload": listens })
}

async fn count(db: &PgPool, table: &str) -> i64 {
    let sql = match table {
        "artist" => "SELECT count(*) FROM artist",
        "release" => "SELECT count(*) FROM release",
        "recording" => "SELECT count(*) FROM recording",
        "listen" => "SELECT count(*) FROM listen",
        _ => unreachable!(),
    };
    sqlx::query_scalar(sql).fetch_one(db).await.unwrap()
}

/// Artists, releases, recordings and listens.
async fn counts(db: &PgPool) -> [i64; 4] {
    [
        count(db, "artist").await,
        count(db, "release").await,
        count(db, "recording").await,
        count(db, "listen").await,
    ]
}

#[sqlx::test(fixtures("profiles"))]
async fn validate_token_names_the_user(db: PgPool) {
    let token = fiona_token(&db).await;
    let valid =
        json!({ "code": 200, "message": "Token valid.", "valid": true, "user_name": "Fiona" });
    let invalid = json!({ "code": 200, "message": "Token invalid.", "valid": false });

    let with_header = |value: String| {
        Request::get(VALIDATE)
            .header(header::AUTHORIZATION, value)
            .body(Body::empty())
            .unwrap()
    };
    assert_eq!(
        send(&db, with_header(format!("Token {token}"))).await,
        (StatusCode::OK, valid.clone())
    );
    assert_eq!(
        get(&db, &format!("{VALIDATE}?token={token}")).await,
        (StatusCode::OK, valid)
    );
    assert_eq!(
        send(
            &db,
            with_header("Token 00000000-0000-0000-0000-000000000000".into())
        )
        .await,
        (StatusCode::OK, invalid.clone())
    );
    assert_eq!(
        get(&db, VALIDATE).await,
        (
            StatusCode::BAD_REQUEST,
            json!({ "code": 400, "error": "You need to provide an Authorization token." })
        )
    );

    let listed = tokens::list(&db, Some("FIONA")).await.unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].label, "Navidrome");
    assert!(listed[0].last_used_at.is_some());

    assert!(tokens::revoke(&db, listed[0].id).await.unwrap());
    assert!(
        !tokens::revoke(&db, listed[0].id).await.unwrap(),
        "already revoked"
    );
    assert_eq!(
        send(&db, with_header(format!("Token {token}"))).await,
        (StatusCode::OK, invalid)
    );
}

#[derive(sqlx::FromRow)]
struct StoredListen {
    artist_raw: String,
    track_raw: String,
    album_raw: Option<String>,
    track_number: Option<i16>,
    duration_ms: Option<i32>,
    client: Option<String>,
    mbid: Option<String>,
    artist_id: i64,
    recording_id: i64,
    release_id: Option<i64>,
}

#[sqlx::test(fixtures("profiles"))]
async fn a_listen_finds_its_catalog_entries_and_keeps_the_raw_data(db: PgPool) {
    let token = fiona_token(&db).await;
    let before = counts(&db).await;
    let listened_at = now() - 100;

    submit_ok(
        &db,
        &token,
        &single(json!({
            "listened_at": listened_at,
            "track_metadata": {
                "artist_name": "die ärzte",
                "track_name": "UNROCKBAR ",
                "release_name": "Geräusch",
                "additional_info": {
                    "submission_client": "Navidrome",
                    "submission_client_version": "0.58.0",
                    "tracknumber": 3,
                    "duration_ms": 196_000,
                    "recording_mbid": "6a0c5c43-7d0e-4f5c-a0a4-49d3e1d6b6b2",
                },
            },
        })),
    )
    .await;

    let row: StoredListen = sqlx::query_as(
        "SELECT artist_raw, track_raw, album_raw, track_number, duration_ms, client,
                extra->>'recording_mbid' AS mbid, artist_id, recording_id, release_id
           FROM listen WHERE profile_id = 1 AND listened_at = to_timestamp($1)",
    )
    .bind(listened_at as f64)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(
        (
            row.artist_raw.as_str(),
            row.track_raw.as_str(),
            row.album_raw.as_deref()
        ),
        ("die ärzte", "UNROCKBAR ", Some("Geräusch"))
    );
    assert_eq!(
        (row.artist_id, row.recording_id, row.release_id),
        (1, 1, Some(1))
    );
    assert_eq!(row.track_number, Some(3));
    assert_eq!(row.duration_ms, Some(196_000));
    assert_eq!(row.client.as_deref(), Some("Navidrome 0.58.0"));
    assert_eq!(
        row.mbid.as_deref(),
        Some("6a0c5c43-7d0e-4f5c-a0a4-49d3e1d6b6b2")
    );

    let [artists, releases, recordings, listens] = before;
    assert_eq!(
        counts(&db).await,
        [artists, releases, recordings, listens + 1]
    );

    let (_, overview) = get(&db, "/api/profiles/fiona/default").await;
    assert_eq!(overview["listens"], 9);
    let (_, recent) = get(&db, "/api/profiles/fiona/default/listens?limit=1").await;
    assert_eq!(
        recent["listens"][0],
        json!({
            "listened_at": OffsetDateTime::from_unix_timestamp(listened_at)
                .unwrap()
                .format(&time::format_description::well_known::Rfc3339)
                .unwrap(),
            "artist": "Die Ärzte",
            "track": "Unrockbar",
            "album": "Geräusch",
        })
    );
}

#[sqlx::test(fixtures("profiles"))]
async fn new_names_create_catalog_entries_once(db: PgPool) {
    let token = fiona_token(&db).await;
    let [artists, releases, recordings, listens] = counts(&db).await;
    let t = now() - 1000;

    submit_ok(
        &db,
        &token,
        &import(vec![
            listen(
                t,
                "Wir sind Helden",
                "Nur ein Wort",
                Some("Die Reklamation"),
            ),
            listen(t + 200, " wir sind  HELDEN", "nur ein wort", None),
            listen(
                t + 400,
                "Wir sind Helden",
                "Guten Tag",
                Some("die reklamation"),
            ),
            listen(t + 600, "Die Ärzte", "Junge", Some("Jazz ist anders")),
        ]),
    )
    .await;

    // One new artist, three new recordings, two new releases.
    assert_eq!(
        counts(&db).await,
        [artists + 1, releases + 2, recordings + 3, listens + 4]
    );
    let names: Vec<String> = sqlx::query_scalar("SELECT name FROM artist ORDER BY id")
        .fetch_all(&db)
        .await
        .unwrap();
    assert_eq!(names.last().unwrap(), "Wir sind Helden");

    let (_, top) = get(
        &db,
        &format!("/api/profiles/fiona/default/top/artists?year={}", year(t)),
    )
    .await;
    assert_eq!(top[0]["name"], "Wir sind Helden");
    assert_eq!(top[0]["listens"], 3);

    // Later listens with the same names reuse the new entries.
    submit_ok(
        &db,
        &token,
        &single(listen(
            t + 800,
            "WIR SIND HELDEN",
            "Guten Tag",
            Some("Die Reklamation"),
        )),
    )
    .await;
    assert_eq!(
        counts(&db).await,
        [artists + 1, releases + 2, recordings + 3, listens + 5]
    );
}

fn year(unix: i64) -> i32 {
    OffsetDateTime::from_unix_timestamp(unix).unwrap().year()
}

#[sqlx::test(fixtures("profiles"))]
async fn resubmitted_listens_are_skipped(db: PgPool) {
    let token = fiona_token(&db).await;
    let listens = count(&db, "listen").await;
    let t = now() - 1000;
    let batch = import(vec![
        listen(t, "Björk", "Jóga", None),
        listen(t + 300, "Björk", "Human Behaviour", Some("Debut")),
        // The fixture already has a listen at 2016-03-04 12:00 UTC.
        listen(1_457_092_800, "Björk", "Jóga", None),
    ]);

    submit_ok(&db, &token, &batch).await;
    assert_eq!(count(&db, "listen").await, listens + 2);
    submit_ok(&db, &token, &batch).await;
    assert_eq!(count(&db, "listen").await, listens + 2);
}

#[sqlx::test(fixtures("profiles"))]
async fn playing_now_shows_on_the_profile_until_it_runs_out(db: PgPool) {
    let token = fiona_token(&db).await;
    let url = "/api/profiles/fiona/default/now-playing";
    assert_eq!(get(&db, url).await, (StatusCode::OK, Value::Null));
    let listens = count(&db, "listen").await;

    let playing_now = |track: &str, info: Value| {
        json!({
            "listen_type": "playing_now",
            "payload": [{ "track_metadata": {
                "artist_name": "Die Ärzte",
                "track_name": track,
                "release_name": "Geräusch",
                "additional_info": info,
            }}],
        })
    };
    submit_ok(
        &db,
        &token,
        &playing_now("Unrockbar", json!({ "duration_ms": 196_000 })),
    )
    .await;

    let (status, playing) = get(&db, url).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        (
            &playing["artist"],
            &playing["track"],
            &playing["album"],
            &playing["duration_ms"]
        ),
        (
            &json!("Die Ärzte"),
            &json!("Unrockbar"),
            &json!("Geräusch"),
            &json!(196_000)
        )
    );
    assert!(playing["started_at"].is_string());
    assert_eq!(
        count(&db, "listen").await,
        listens,
        "now playing is not a listen"
    );

    // The next track replaces it; without a length it shows for ten minutes.
    submit_ok(&db, &token, &playing_now("Deine Schuld", json!({}))).await;
    let (_, playing) = get(&db, url).await;
    assert_eq!(playing["track"], "Deine Schuld");
    let shown_for: i32 = sqlx::query_scalar(
        "SELECT extract(epoch FROM expires_at - started_at)::int FROM now_playing",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(shown_for, 600);

    sqlx::query("UPDATE now_playing SET expires_at = now() - interval '1 second'")
        .execute(&db)
        .await
        .unwrap();
    assert_eq!(get(&db, url).await, (StatusCode::OK, Value::Null));
}

#[sqlx::test(fixtures("profiles"))]
async fn a_token_scrobbles_to_its_own_profile(db: PgPool) {
    let work = tokens::create(&db, "Fiona", "Arbeit", "Laptop")
        .await
        .unwrap();
    submit_ok(
        &db,
        &work.token,
        &single(listen(now() - 60, "Björk", "Jóga", None)),
    )
    .await;

    let per_profile: Vec<(i64, i64)> =
        sqlx::query_as("SELECT profile_id, count(*) FROM listen GROUP BY 1 ORDER BY 1")
            .fetch_all(&db)
            .await
            .unwrap();
    assert_eq!(per_profile, [(1, 8), (2, 2), (3, 1)]);

    // The private profile stays hidden, including what it is playing.
    assert_eq!(
        get(&db, "/api/profiles/fiona/arbeit/now-playing").await.0,
        StatusCode::NOT_FOUND
    );

    let err = tokens::create(&db, "fiona", "nope", "x")
        .await
        .err()
        .unwrap();
    assert_eq!(err.to_string(), "user fiona has no profile nope");
    assert!(tokens::create(&db, "nobody", "default", "x").await.is_err());
}

#[sqlx::test(fixtures("profiles"))]
async fn requests_without_a_valid_token_are_refused(db: PgPool) {
    let token = fiona_token(&db).await;
    let body = single(listen(now() - 60, "Björk", "Jóga", None)).to_string();
    let request = |authorization: Option<&str>| {
        let mut request = Request::post(SUBMIT);
        if let Some(value) = authorization {
            request = request.header(header::AUTHORIZATION, value);
        }
        request.body(Body::from(body.clone())).unwrap()
    };
    let refused = |message: &str| {
        (
            StatusCode::UNAUTHORIZED,
            json!({ "code": 401, "error": message }),
        )
    };

    assert_eq!(
        send(&db, request(None)).await,
        refused("You need to provide an Authorization header.")
    );
    assert_eq!(
        send(&db, request(Some(&format!("Basic {token}")))).await,
        refused("Provided Authorization header is invalid.")
    );
    assert_eq!(
        send(
            &db,
            request(Some("Token 00000000-0000-0000-0000-000000000000"))
        )
        .await,
        refused("Invalid authorization token.")
    );
    let id = tokens::list(&db, None).await.unwrap()[0].id;
    tokens::revoke(&db, id).await.unwrap();
    assert_eq!(
        send(&db, request(Some(&format!("Token {token}")))).await,
        refused("Invalid authorization token.")
    );
    assert_eq!(count(&db, "listen").await, 10, "nothing was stored");
}

#[sqlx::test(fixtures("profiles"))]
async fn bad_requests_get_listenbrainz_errors(db: PgPool) {
    let token = fiona_token(&db).await;
    let too_old = single(listen(1_000_000_000, "Björk", "Jóga", None));
    let (status, error) = submit(&db, &token, &too_old).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(error["code"], 400);
    assert!(
        error["error"]
            .as_str()
            .unwrap()
            .contains("listened_at is too low")
    );

    // No Content-Type is fine, like on ListenBrainz.
    let request = Request::post(SUBMIT)
        .header(header::AUTHORIZATION, format!("Token {token}"))
        .body(Body::from(
            single(listen(now() - 60, "Björk", "Jóga", None)).to_string(),
        ))
        .unwrap();
    assert_eq!(send(&db, request).await.0, StatusCode::OK);

    let huge = format!("{{\"padding\": \"{}\"}}", "x".repeat(10_240_000));
    let request = Request::post(SUBMIT)
        .header(header::AUTHORIZATION, format!("Token {token}"))
        .body(Body::from(huge))
        .unwrap();
    let (status, error) = send(&db, request).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        error["error"]
            .as_str()
            .unwrap()
            .starts_with("Payload too large")
    );
}

#[sqlx::test(fixtures("profiles"))]
async fn concurrent_listens_share_new_catalog_entries(db: PgPool) {
    let token = fiona_token(&db).await;
    let [artists, releases, recordings, listens] = counts(&db).await;
    let t = now() - 1000;

    let mut tasks = tokio::task::JoinSet::new();
    for i in 0..10 {
        let (db, token) = (db.clone(), token.clone());
        tasks.spawn(async move {
            let body = single(listen(
                t + i,
                "Kettcar",
                "Landungsbrücken raus",
                Some("Du und wieviel von deinen Freunden"),
            ));
            submit(&db, &token, &body).await.0
        });
    }
    while let Some(status) = tasks.join_next().await {
        assert_eq!(status.unwrap(), StatusCode::OK);
    }

    assert_eq!(
        counts(&db).await,
        [artists + 1, releases + 1, recordings + 1, listens + 10]
    );
}
