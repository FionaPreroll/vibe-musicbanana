//! Searching what a profile has heard.

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

// fixtures/profiles.sql: Fiona's default profile has heard Die Ärzte (Unrockbar 3×,
// Deine Schuld 1×, from Geräusch), Björk (Human Behaviour 2×, Jóga) and Tiësto;
// her private profile Tiësto, and alex Deine Schuld.

async fn search(db: &PgPool, profile: &str, q: &str) -> (StatusCode, Value) {
    let app = router(AppState { db: db.clone() }, Path::new("does-not-exist"));
    let q: String = url_escape(q);
    let uri = format!("/api/profiles/{profile}/search?q={q}");
    let res = app
        .oneshot(Request::get(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = res.status();
    let body = res.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&body).unwrap_or(Value::Null))
}

fn url_escape(q: &str) -> String {
    q.bytes()
        .map(|b| match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'.' | b'_' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// `[name, listens]` per artist and `[name, artist, listens]` per album and track.
async fn found(db: &PgPool, profile: &str, q: &str) -> Value {
    let (status, json) = search(db, profile, q).await;
    assert_eq!(status, StatusCode::OK, "{q}");
    let entries = |kind: &str| -> Vec<Value> {
        json[kind]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| match e.get("artist") {
                Some(artist) => json!([e["name"], artist, e["listens"]]),
                None => json!([e["name"], e["listens"]]),
            })
            .collect()
    };
    json!({
        "artists": entries("artists"),
        "releases": entries("releases"),
        "recordings": entries("recordings"),
    })
}

#[sqlx::test(fixtures("profiles"))]
async fn finds_names_without_minding_case_or_accents(db: PgPool) {
    assert_eq!(
        found(&db, "fiona/default", "arzte").await,
        json!({
            "artists": [["Die Ärzte", 4]],
            "releases": [["Geräusch", "Die Ärzte", 3]],
            "recordings": [["Unrockbar", "Die Ärzte", 3], ["Deine Schuld", "Die Ärzte", 1]],
        })
    );
    assert_eq!(
        found(&db, "fiona/default", "JOGA").await["recordings"],
        json!([["Jóga", "Björk", 1]])
    );
    assert_eq!(
        found(&db, "fiona/default", "tie").await["artists"],
        json!([["Tiësto", 1]])
    );
}

#[sqlx::test(fixtures("profiles"))]
async fn every_word_counts_and_may_name_the_artist(db: PgPool) {
    assert_eq!(
        found(&db, "fiona/default", "  Ärzte   unrock ").await,
        json!({
            "artists": [],
            "releases": [],
            "recordings": [["Unrockbar", "Die Ärzte", 3]],
        })
    );
    assert_eq!(
        found(&db, "fiona/default", "björk behaviour").await["recordings"],
        json!([["Human Behaviour", "Björk", 2]])
    );
    // Symbols are plain text, not patterns.
    assert_eq!(
        found(&db, "fiona/default", "%").await,
        json!({"artists": [], "releases": [], "recordings": []})
    );
}

#[sqlx::test(fixtures("profiles"))]
async fn only_what_the_profile_has_heard(db: PgPool) {
    assert_eq!(
        found(&db, "alex/default", "ärzte").await,
        json!({
            "artists": [["Die Ärzte", 1]],
            "releases": [],
            "recordings": [["Deine Schuld", "Die Ärzte", 1]],
        })
    );
    assert_eq!(
        found(&db, "alex/default", "").await,
        json!({"artists": [], "releases": [], "recordings": []})
    );
    // A private profile is not there for strangers.
    assert_eq!(
        search(&db, "Fiona/arbeit", "tiesto").await.0,
        StatusCode::NOT_FOUND
    );
}
