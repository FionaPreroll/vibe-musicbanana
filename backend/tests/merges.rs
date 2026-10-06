//! The page "Merges" for admins: suggestions, hiding them, merging and undoing.

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

// fixtures/merge.sql: "Die Aerzte" (artist 4) next to Die Ärzte (1), "Bjork"
// (5) next to Björk (2), "Unrockbar (Live)" (recording 8) next to Unrockbar (1).

async fn call(
    app: &Router,
    method: Method,
    uri: &str,
    cookie: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value, Option<String>) {
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
        .map(|v| v.to_str().unwrap().split(';').next().unwrap().to_owned());
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body = serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into_owned()));
    (status, body, cookie)
}

/// The app with Fiona as admin, and the login cookies of Fiona and alex.
async fn setup(db: &PgPool) -> (Router, String, String) {
    sqlx::query("UPDATE account SET password_hash = $1, is_admin = (username = 'Fiona')")
        .bind(auth::hash_password("banana-split").unwrap())
        .execute(db)
        .await
        .unwrap();
    let app = router(AppState::new(db.clone()), Path::new("does-not-exist"));
    let mut cookies = Vec::new();
    for name in ["fiona", "alex"] {
        let body = json!({"login": name, "password": "banana-split"});
        let (_, _, cookie) = call(&app, Method::POST, "/api/session", None, Some(body)).await;
        cookies.push(cookie.unwrap());
    }
    let alex = cookies.pop().unwrap();
    let fiona = cookies.pop().unwrap();
    (app, fiona, alex)
}

fn pairs(suggestions: &Value) -> Vec<(i64, i64)> {
    suggestions["suggestions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| {
            (
                s["from"]["id"].as_i64().unwrap(),
                s["into"]["id"].as_i64().unwrap(),
            )
        })
        .collect()
}

async fn listens_of_artist(db: &PgPool, id: i64) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM listen WHERE artist_id = $1")
        .bind(id)
        .fetch_one(db)
        .await
        .unwrap()
}

#[sqlx::test(fixtures("profiles", "merge"))]
async fn only_admins_merge(db: PgPool) {
    let (app, _, alex) = setup(&db).await;
    for (method, uri, body) in [
        (Method::GET, "/api/admin/merges", None),
        (Method::GET, "/api/admin/merges/suggestions/artist", None),
        (Method::GET, "/api/admin/catalog/artist?q=aerzte", None),
        (
            Method::POST,
            "/api/admin/merges",
            Some(json!({"kind": "artist", "from": 4, "into": 1})),
        ),
        (Method::POST, "/api/admin/merges/1/undo", Some(json!({}))),
    ] {
        let without = call(&app, method.clone(), uri, None, body.clone()).await.0;
        assert_eq!(without, StatusCode::UNAUTHORIZED, "{method} {uri}");
        let user = call(&app, method.clone(), uri, Some(&alex), body).await.0;
        assert_eq!(user, StatusCode::FORBIDDEN, "{method} {uri}");
    }
    assert_eq!(listens_of_artist(&db, 4).await, 2);
}

#[sqlx::test(fixtures("profiles", "merge"))]
async fn suggest_preview_merge_and_undo(db: PgPool) {
    let (app, fiona, _) = setup(&db).await;
    let fiona = Some(fiona.as_str());

    let (status, found, _) = call(
        &app,
        Method::GET,
        "/api/admin/merges/suggestions/artist",
        fiona,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(pairs(&found).contains(&(4, 1)), "{found}");
    assert_eq!(found["suggestions"][0]["likeness"], "same_letters");

    // Found by name, or by id.
    let (_, hits, _) = call(
        &app,
        Method::GET,
        "/api/admin/catalog/artist?q=aerzte",
        fiona,
        None,
    )
    .await;
    assert_eq!(hits[0]["id"], 4);
    assert_eq!(hits[0]["listens"], 2);
    let (_, hits, _) = call(
        &app,
        Method::GET,
        "/api/admin/catalog/artist?q=%231",
        fiona,
        None,
    )
    .await;
    assert_eq!(hits[0]["name"], "Die Ärzte");

    // A dry run changes nothing.
    let merge = json!({"kind": "artist", "from": 4, "into": 1, "dry_run": true});
    let (status, preview, _) =
        call(&app, Method::POST, "/api/admin/merges", fiona, Some(merge)).await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    assert_eq!(preview["dry_run"], true);
    assert_eq!(preview["listens"], 2);
    assert_eq!(preview["op"], Value::Null);
    assert_eq!(preview["told_apart"], false);
    assert_eq!(listens_of_artist(&db, 4).await, 2);
    let (_, log, _) = call(&app, Method::GET, "/api/admin/merges", fiona, None).await;
    assert_eq!(log["merges"], json!([]));

    let merge = json!({"kind": "artist", "from": 4, "into": 1});
    let (status, merged, _) = call(
        &app,
        Method::POST,
        "/api/admin/merges",
        fiona,
        Some(merge.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{merged}");
    let op = merged["op"].as_i64().unwrap();
    assert_eq!(listens_of_artist(&db, 4).await, 0);

    // Twice is refused, with the reason.
    let (status, again, _) =
        call(&app, Method::POST, "/api/admin/merges", fiona, Some(merge)).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(
        again.as_str().unwrap().contains("merged into 1 already"),
        "{again}"
    );

    let (_, log, _) = call(&app, Method::GET, "/api/admin/merges", fiona, None).await;
    assert_eq!(log["merges"][0]["id"], op);
    assert_eq!(log["merges"][0]["from"], json!([4, "Die Aerzte"]));
    assert_eq!(log["merges"][0]["undone_at"], Value::Null);

    let undo = format!("/api/admin/merges/{op}/undo");
    let (status, preview, _) = call(
        &app,
        Method::POST,
        &undo,
        fiona,
        Some(json!({"dry_run": true})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    assert_eq!(preview["dry_run"], true);
    assert_eq!(listens_of_artist(&db, 4).await, 0);

    let (status, undone, _) = call(&app, Method::POST, &undo, fiona, Some(json!({}))).await;
    assert_eq!(status, StatusCode::OK, "{undone}");
    assert_eq!(listens_of_artist(&db, 4).await, 2);
    let (_, log, _) = call(&app, Method::GET, "/api/admin/merges", fiona, None).await;
    assert!(log["merges"][0]["undone_at"].is_string());

    let (status, _, _) = call(&app, Method::POST, &undo, fiona, Some(json!({}))).await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (status, _, _) = call(
        &app,
        Method::POST,
        "/api/admin/merges/999/undo",
        fiona,
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test(fixtures("profiles", "merge"))]
async fn hidden_suggestions_stay_away(db: PgPool) {
    let (app, fiona, _) = setup(&db).await;
    let fiona = Some(fiona.as_str());
    let suggestions = "/api/admin/merges/suggestions/recording";

    let (_, found, _) = call(&app, Method::GET, suggestions, fiona, None).await;
    assert!(pairs(&found).contains(&(8, 1)), "{found}");

    let (status, _, _) = call(
        &app,
        Method::POST,
        "/api/admin/merges/hidden/recording",
        fiona,
        Some(json!({"from": 8, "into": 1})),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, found, _) = call(&app, Method::GET, suggestions, fiona, None).await;
    assert!(!pairs(&found).contains(&(8, 1)), "{found}");
    assert_eq!(found["hidden"], 1);

    let (_, hidden, _) = call(
        &app,
        Method::GET,
        "/api/admin/merges/hidden/recording",
        fiona,
        None,
    )
    .await;
    assert_eq!(hidden[0]["from"], json!([8, "Unrockbar (Live)"]));
    assert_eq!(hidden[0]["into"], json!([1, "Unrockbar"]));

    // Also when asked the other way round.
    let (status, _, _) = call(
        &app,
        Method::DELETE,
        "/api/admin/merges/hidden/recording/1/8",
        fiona,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, found, _) = call(&app, Method::GET, suggestions, fiona, None).await;
    assert!(pairs(&found).contains(&(8, 1)), "{found}");
}

#[sqlx::test(fixtures("profiles", "merge"))]
async fn musicbrainz_ids_need_force(db: PgPool) {
    let (app, fiona, _) = setup(&db).await;
    let fiona = Some(fiona.as_str());
    sqlx::query(
        "INSERT INTO artist_mbid (artist_id, mbid, name_key) VALUES
             (4, 'f2fb0ff0-5679-42ec-a55c-15109ce6e320', 'die aerzte'),
             (1, '00000000-0000-0000-0000-000000000001', 'die ärzte')",
    )
    .execute(&db)
    .await
    .unwrap();

    let (_, preview, _) = call(
        &app,
        Method::POST,
        "/api/admin/merges",
        fiona,
        Some(json!({"kind": "artist", "from": 4, "into": 1, "dry_run": true})),
    )
    .await;
    assert_eq!(preview["told_apart"], true, "{preview}");

    let merge = |force: bool| json!({"kind": "artist", "from": 4, "into": 1, "force": force});
    let (status, _, _) = call(
        &app,
        Method::POST,
        "/api/admin/merges",
        fiona,
        Some(merge(false)),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (status, merged, _) = call(
        &app,
        Method::POST,
        "/api/admin/merges",
        fiona,
        Some(merge(true)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{merged}");
    assert_eq!(listens_of_artist(&db, 4).await, 0);
}
