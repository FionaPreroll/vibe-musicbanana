//! Importing Spotify plays from YourSpotify, against a stand-in for its API.

use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use axum::{
    Json, Router,
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};
use musicbanana::yourspotify::{self, CLIENT, Report, Source};
use serde_json::{Value, json};
use sqlx::PgPool;
use time::{Duration, OffsetDateTime, format_description::well_known::Rfc3339, macros::datetime};
use tokio::net::TcpListener;

// fixtures/profiles.sql: Fiona's default profile is 1, with Die Ärzte (artist 1),
// their album Geräusch (release 1) and its track Unrockbar (recording 1).

/// Made up; a real one is a UUID too.
const TOKEN: &str = "0b7e4c55-2f0a-4d6e-9c3b-5a1d8e6f7a90";

/// What YourSpotify's `GET /spotify/gethistory` does with the parts used here.
#[derive(Default)]
struct YourSpotify {
    /// Newest first.
    plays: Mutex<Vec<Value>>,
    /// Plays fetched from Spotify once a page with plays has been asked for.
    meanwhile: Mutex<Vec<Value>>,
    /// The offsets asked for, with the start of the time range.
    asked: Mutex<Vec<(usize, Option<OffsetDateTime>)>>,
    /// An older YourSpotify, which takes no time range.
    no_range: AtomicBool,
    /// Fails for time ranges that start at this time or later.
    down_from: Mutex<Option<OffsetDateTime>>,
}

fn played_at(play: &Value) -> OffsetDateTime {
    OffsetDateTime::parse(play["played_at"].as_str().unwrap(), &Rfc3339).unwrap()
}

async fn history(
    State(server): State<Arc<YourSpotify>>,
    Query(query): Query<HashMap<String, String>>,
) -> Response {
    if query.get("token").map(String::as_str) != Some(TOKEN) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"code": "NOT_LOGGED"})),
        )
            .into_response();
    }
    let number: usize = query["number"].parse().unwrap();
    let offset: usize = query["offset"].parse().unwrap();
    if number > 20 {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let time = |name: &str| {
        query
            .get(name)
            .map(|text| OffsetDateTime::parse(text, &Rfc3339).unwrap())
    };
    let (start, end) = (time("start"), time("end"));
    server.asked.lock().unwrap().push((offset, start));
    if let (Some(start), Some(down)) = (start, *server.down_from.lock().unwrap())
        && start >= down
    {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    // As YourSpotify: only with both ends, and without them.
    let range = match (start, end) {
        (Some(start), Some(end)) if !server.no_range.load(Ordering::Relaxed) => Some((start, end)),
        _ => None,
    };
    let mut plays = server.plays.lock().unwrap();
    let page: Vec<Value> = plays
        .iter()
        .filter(|play| {
            range.is_none_or(|(start, end)| played_at(play) > start && played_at(play) < end)
        })
        .skip(offset)
        .take(number)
        .cloned()
        .collect();
    if !page.is_empty() {
        for play in server.meanwhile.lock().unwrap().drain(..).rev() {
            plays.insert(0, play);
        }
    }
    Json(page).into_response()
}

/// Starts the stand-in and returns the address of its API.
async fn serve(server: &Arc<YourSpotify>) -> String {
    let app = Router::new()
        .route("/api/spotify/gethistory", get(history))
        .route(
            "/web/spotify/gethistory",
            get(|| async { "<!doctype html><title>Your Spotify</title>" }),
        )
        .with_state(server.clone());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://{address}/api/")
}

fn spotify_id(name: &str) -> String {
    name.to_lowercase().replace(' ', "")
}

/// A play as YourSpotify returns it, with a few of the fields not used.
fn play(at: OffsetDateTime, title: &str, artists: &[&str], album: Option<&str>) -> Value {
    let artist_ids: Vec<String> = artists.iter().map(|a| spotify_id(a)).collect();
    // YourSpotify's lookup gives the artists in no particular order.
    let full_artists: Vec<Value> = artists
        .iter()
        .rev()
        .map(|a| json!({"id": spotify_id(a), "name": a, "genres": [], "popularity": 50}))
        .collect();
    json!({
        "_id": "66f0a1b2c3d4e5f6a7b8c9d0",
        "owner": "66f0a1b2c3d4e5f6a7b8c9d1",
        "id": spotify_id(title),
        "albumId": album.map(spotify_id),
        "primaryArtistId": artist_ids.first(),
        "artistIds": artist_ids,
        "durationMs": 215000,
        "played_at": at.format(&Rfc3339).unwrap(),
        "__v": 0,
        "track": {
            "id": spotify_id(title),
            "name": title,
            "artists": artist_ids,
            "album": album.map(spotify_id),
            "duration_ms": 215000,
            "track_number": 3,
            "disc_number": 1,
            "explicit": false,
            "full_album": album.map(|name| json!({"id": spotify_id(name), "name": name, "album_type": "album"})),
            "full_artists": full_artists,
        },
    })
}

/// `n` plays, an hour apart from 2026-01-01 10:00:00.250, newest first.
fn plays(n: i64) -> Vec<Value> {
    (0..n)
        .rev()
        .map(|i| play(at(i), &format!("Song {i}"), &["Björk"], Some("Debut")))
        .collect()
}

fn at(hour: i64) -> OffsetDateTime {
    datetime!(2026-01-01 10:00:00.250 UTC) + Duration::hours(hour)
}

async fn imported(db: &PgPool) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM listen WHERE profile_id = 1 AND client = $1")
        .bind(CLIENT)
        .fetch_one(db)
        .await
        .unwrap()
}

fn asked(server: &YourSpotify) -> Vec<(usize, Option<OffsetDateTime>)> {
    std::mem::take(&mut server.asked.lock().unwrap())
}

/// The pages asked for beyond the first of a time range.
fn further_pages(asked: &[(usize, Option<OffsetDateTime>)]) -> Vec<usize> {
    asked
        .iter()
        .map(|&(offset, _)| offset)
        .filter(|&offset| offset > 0)
        .collect()
}

#[sqlx::test(fixtures("profiles"))]
async fn imports_the_history_and_then_only_new_plays(db: PgPool) {
    let server = Arc::new(YourSpotify::default());
    *server.plays.lock().unwrap() = plays(45);
    let source = Source::new(&serve(&server).await, TOKEN).unwrap();

    let report = yourspotify::import(&db, &source, 1, false).await.unwrap();
    assert_eq!(
        report,
        Report {
            since: None,
            plays: 45,
            recorded: 45,
            without_artist: 0,
            first: Some(at(0)),
            last: Some(at(44)),
        }
    );
    assert_eq!(
        report.to_string(),
        "Found 45 Spotify plays in YourSpotify from 2026-01-01 10:00 UTC to \
         2026-01-03 06:00 UTC: imported 45."
    );
    let first = asked(&server);
    assert!(first.iter().all(|(_, start)| start.is_some()), "{first:?}");
    assert_eq!(further_pages(&first), [20, 40]);
    assert_eq!(imported(&db).await, 45);

    // Then only what is new.
    let newer = [
        play(at(46), "Unrockbar", &["Die Ärzte"], Some("Geräusch")),
        play(at(45), "Song 45", &["Björk"], None),
    ];
    server.plays.lock().unwrap().splice(0..0, newer);
    let report = yourspotify::import(&db, &source, 1, false).await.unwrap();
    assert_eq!(
        (report.since, report.plays, report.recorded),
        (Some(at(44)), 2, 2)
    );
    let again = asked(&server);
    assert_eq!(again[0], (0, Some(at(44))));
    assert!(further_pages(&again).is_empty(), "{again:?}");
    let (artist, release, recording): (i64, Option<i64>, i64) = sqlx::query_as(
        "SELECT artist_id, release_id, recording_id FROM listen
          WHERE profile_id = 1 AND listened_at = $1",
    )
    .bind(at(46))
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!((artist, release, recording), (1, Some(1), 1));

    let report = yourspotify::import(&db, &source, 1, false).await.unwrap();
    assert_eq!(report.plays, 0);
    assert_eq!(
        report.to_string(),
        "No Spotify plays in YourSpotify after 2026-01-03 08:00 UTC."
    );

    // The whole history again: nothing is stored twice.
    let report = yourspotify::import(&db, &source, 1, true).await.unwrap();
    assert_eq!((report.since, report.plays, report.recorded), (None, 47, 0));
    assert!(
        report
            .to_string()
            .ends_with("imported 0, 47 were in the profile already."),
        "{report}"
    );
    assert_eq!(imported(&db).await, 47);
}

#[sqlx::test(fixtures("profiles"))]
async fn a_play_counts_for_its_main_artist(db: PgPool) {
    let server = Arc::new(YourSpotify::default());
    let mut unknown = play(at(1), "Jóga", &["Björk"], Some("Homogenic"));
    unknown["track"]["full_artists"] = json!([]);
    *server.plays.lock().unwrap() = vec![
        unknown,
        play(at(0), "Unrockbar", &["Die Ärzte", "Björk"], None),
    ];
    let source = Source::new(&serve(&server).await, TOKEN).unwrap();

    let report = yourspotify::import(&db, &source, 1, false).await.unwrap();
    assert_eq!(
        (report.plays, report.recorded, report.without_artist),
        (2, 1, 1)
    );
    assert!(
        report
            .to_string()
            .ends_with("imported 1, 1 left out as YourSpotify knows none of their artists."),
        "{report}"
    );
    let (artist, track, album, recording, duration, number, extra): (
        String,
        String,
        Option<String>,
        i64,
        Option<i32>,
        Option<i16>,
        Value,
    ) = sqlx::query_as(
        "SELECT artist_raw, track_raw, album_raw, recording_id, duration_ms, track_number,
                extra
           FROM listen
          WHERE profile_id = 1 AND client = $1",
    )
    .bind(CLIENT)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(
        (
            artist.as_str(),
            track.as_str(),
            album,
            recording,
            duration,
            number
        ),
        ("Die Ärzte", "Unrockbar", None, 1, Some(215000), Some(3))
    );
    assert_eq!(
        extra,
        json!({
            "music_service": "spotify.com",
            "spotify_id": "https://open.spotify.com/track/unrockbar",
            "spotify_artist_ids": [
                "https://open.spotify.com/artist/dieärzte",
                "https://open.spotify.com/artist/björk",
            ],
            "artist_names": ["Die Ärzte", "Björk"],
        })
    );
}

#[sqlx::test(fixtures("profiles"))]
async fn plays_that_come_in_meanwhile_are_imported_once(db: PgPool) {
    let server = Arc::new(YourSpotify::default());
    *server.plays.lock().unwrap() = plays(25);
    // Two new plays move the others down while the import is under way, so the
    // second page starts with two from the first.
    *server.meanwhile.lock().unwrap() = vec![
        play(at(26), "Song 26", &["Björk"], None),
        play(at(25), "Song 25", &["Björk"], None),
    ];
    let source = Source::new(&serve(&server).await, TOKEN).unwrap();

    let report = yourspotify::import(&db, &source, 1, false).await.unwrap();
    assert_eq!((report.plays, report.recorded), (25, 25));
    assert_eq!(further_pages(&asked(&server)), [20]);
    let report = yourspotify::import(&db, &source, 1, false).await.unwrap();
    assert_eq!((report.plays, report.recorded), (2, 2));
    assert_eq!(imported(&db).await, 27);
}

#[sqlx::test(fixtures("profiles"))]
async fn an_import_that_stops_goes_on_where_it_stopped(db: PgPool) {
    let server = Arc::new(YourSpotify::default());
    // Twelve plays 40 days apart, more than one window.
    let day = |n: i64| datetime!(2025-01-01 20:00 UTC) + Duration::days(40 * n);
    *server.plays.lock().unwrap() = (0..12)
        .rev()
        .map(|n| play(day(n), &format!("Song {n}"), &["Björk"], None))
        .collect();
    *server.down_from.lock().unwrap() = Some(day(6));
    let source = Source::new(&serve(&server).await, TOKEN).unwrap();

    let error = yourspotify::import(&db, &source, 1, false)
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("answered 500"), "{error:#}");
    // The plays before are in the profile already, without a gap.
    let stored: Vec<OffsetDateTime> = sqlx::query_scalar(
        "SELECT listened_at FROM listen WHERE profile_id = 1 AND client = $1
          ORDER BY listened_at",
    )
    .bind(CLIENT)
    .fetch_all(&db)
    .await
    .unwrap();
    assert!(!stored.is_empty() && stored.len() < 12, "{stored:?}");
    assert_eq!(
        stored,
        (0..stored.len() as i64).map(day).collect::<Vec<_>>()
    );

    *server.down_from.lock().unwrap() = None;
    let report = yourspotify::import(&db, &source, 1, false).await.unwrap();
    assert_eq!(
        (report.since, report.plays, report.recorded),
        (
            stored.last().copied(),
            12 - stored.len(),
            12 - stored.len() as u64
        )
    );
    assert_eq!(imported(&db).await, 12);
}

#[sqlx::test(fixtures("profiles"))]
async fn plays_at_the_edge_of_a_window_come_once(db: PgPool) {
    let server = Arc::new(YourSpotify::default());
    // The first window ends with 2007, the next one is 30 days long.
    let edges = [
        datetime!(2008-01-31 0:00 UTC),
        datetime!(2008-01-01 0:00 UTC),
        datetime!(2007-12-31 23:59:59.999 UTC),
    ];
    *server.plays.lock().unwrap() = edges
        .iter()
        .enumerate()
        .map(|(n, &at)| play(at, &format!("Song {n}"), &["Björk"], None))
        .collect();
    let source = Source::new(&serve(&server).await, TOKEN).unwrap();

    let report = yourspotify::import(&db, &source, 1, false).await.unwrap();
    assert_eq!((report.plays, report.recorded), (3, 3));
}

#[sqlx::test(fixtures("profiles"))]
async fn an_older_yourspotify_gives_everything_at_once(db: PgPool) {
    let server = Arc::new(YourSpotify::default());
    server.no_range.store(true, Ordering::Relaxed);
    *server.plays.lock().unwrap() = plays(45);
    let source = Source::new(&serve(&server).await, TOKEN).unwrap();

    let report = yourspotify::import(&db, &source, 1, false).await.unwrap();
    assert_eq!((report.plays, report.recorded), (45, 45));
    let epoch = OffsetDateTime::UNIX_EPOCH - Duration::milliseconds(1);
    assert_eq!(
        asked(&server),
        [(0, Some(epoch)), (0, None), (20, None), (40, None)]
    );

    server
        .plays
        .lock()
        .unwrap()
        .insert(0, play(at(45), "Song 45", &["Björk"], None));
    let report = yourspotify::import(&db, &source, 1, false).await.unwrap();
    assert_eq!(
        (report.since, report.plays, report.recorded),
        (Some(at(44)), 1, 1)
    );
    assert_eq!(asked(&server), [(0, Some(at(44))), (0, None)]);
}

#[sqlx::test(fixtures("profiles"))]
async fn refuses_without_showing_the_token(db: PgPool) {
    let server = Arc::new(YourSpotify::default());
    let api = serve(&server).await;
    let error = |result: anyhow::Result<Report>| {
        let message = format!("{:#}", result.unwrap_err());
        assert!(!message.contains(TOKEN), "{message}");
        message
    };

    let wrong = Source::new(&api, "4d1c9a2e-0000-4000-8000-000000000000").unwrap();
    assert!(
        error(yourspotify::import(&db, &wrong, 1, false).await)
            .contains("does not take the token; the public token is in YourSpotify's settings")
    );

    let web = Source::new(&api.replace("/api/", "/web"), TOKEN).unwrap();
    assert!(
        error(yourspotify::import(&db, &web, 1, false).await)
            .contains("has to be the address of YourSpotify's API (API_ENDPOINT)")
    );

    // Nothing listens there any more.
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let gone = format!("http://{}", listener.local_addr().unwrap());
    drop(listener);
    let gone = Source::new(&gone, TOKEN).unwrap();
    assert!(
        error(yourspotify::import(&db, &gone, 1, false).await)
            .starts_with("asking YourSpotify at http://127.0.0.1:")
    );

    for (api, token, message) in [
        (
            "yourspotify:8080",
            TOKEN,
            "yourspotify:8080 is not an http(s) address",
        ),
        ("http://", TOKEN, "http:// is not an address"),
        (
            "http://yourspotify:8080",
            " ",
            "the YourSpotify token is empty",
        ),
    ] {
        let Err(wrong) = Source::new(api, token) else {
            panic!("{api}")
        };
        assert_eq!(wrong.to_string(), message);
    }
    assert_eq!(
        yourspotify::profile_id(&db, "fiona", "DEFAULT")
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        yourspotify::profile_id(&db, "Fiona", "urlaub")
            .await
            .unwrap_err()
            .to_string(),
        "user Fiona has no profile urlaub"
    );
}
