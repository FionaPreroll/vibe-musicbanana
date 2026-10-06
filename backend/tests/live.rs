use std::{path::Path, time::Duration};

use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use http_body_util::BodyExt;
use musicbanana::{
    AppState,
    live::{self, Change},
    router, tokens,
};
use serde_json::json;
use sqlx::PgPool;
use time::OffsetDateTime;
use tower::ServiceExt;

// fixtures/profiles.sql: Fiona's default profile (id 1) is public, her profile
// "arbeit" (id 2) private.

/// The next event name of the stream, or None when nothing came for a while.
/// Each event needs data too, or browsers drop it.
async fn next_event(body: &mut Body) -> Option<String> {
    let mut text = String::new();
    loop {
        let frame = tokio::time::timeout(Duration::from_millis(500), body.frame())
            .await
            .ok()??
            .unwrap();
        if let Ok(data) = frame.into_data() {
            text.push_str(std::str::from_utf8(&data).unwrap());
        }
        if text.contains("\n\n") {
            let field = |name: &str| {
                text.lines()
                    .find_map(|line| line.strip_prefix(name))
                    .map(str::trim)
                    .unwrap_or("")
                    .to_owned()
            };
            let (event, data) = (field("event:"), field("data:"));
            assert!(!data.is_empty(), "no data in {text:?}");
            return Some(event);
        }
    }
}

#[sqlx::test(fixtures("profiles"))]
async fn a_profile_page_hears_what_changes(db: PgPool) {
    let state = AppState::new(db.clone());
    state.live.spawn_listener(db.clone());
    let app = router(state.clone(), Path::new("does-not-exist"));

    let res = app
        .clone()
        .oneshot(
            Request::get("/api/profiles/fiona/default/live")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.headers()[header::CONTENT_TYPE], "text/event-stream");
    let mut body = res.into_body();

    // The listener connects in the background; notify until it hears.
    let mut heard = None;
    for _ in 0..20 {
        live::notify(&db, 1, Change::NowPlaying).await.unwrap();
        heard = next_event(&mut body).await;
        if heard.is_some() {
            break;
        }
    }
    assert_eq!(heard.as_deref(), Some("now-playing"));

    // Another profile's changes stay away; a scrobble to this one arrives.
    live::notify(&db, 2, Change::Listens).await.unwrap();
    let token = tokens::create(&db, "fiona", "default", "Navidrome")
        .await
        .unwrap()
        .token;
    let listen = json!({
        "listen_type": "single",
        "payload": [{
            "listened_at": OffsetDateTime::now_utc().unix_timestamp() - 60,
            "track_metadata": { "artist_name": "Die Ärzte", "track_name": "Unrockbar" },
        }],
    });
    let submitted = app
        .clone()
        .oneshot(
            Request::post("/api/listenbrainz/1/submit-listens")
                .header(header::AUTHORIZATION, format!("Token {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(listen.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(submitted.status(), StatusCode::OK);
    assert_eq!(next_event(&mut body).await.as_deref(), Some("listens"));

    // A shutdown ends the stream.
    state.live.close();
    let end = tokio::time::timeout(Duration::from_secs(2), body.frame()).await;
    assert!(matches!(end, Ok(None)), "the stream should end");
}

#[sqlx::test(fixtures("profiles"))]
async fn a_hidden_profile_has_no_stream(db: PgPool) {
    let app = router(AppState::new(db), Path::new("does-not-exist"));
    let res = app
        .oneshot(
            Request::get("/api/profiles/fiona/arbeit/live")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}
