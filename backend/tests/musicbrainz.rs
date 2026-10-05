//! Artists, albums and tracks of the same name told apart by the MusicBrainz IDs
//! that come with scrobbles, and merges that go by them.

use std::{
    path::Path,
    sync::atomic::{AtomicI64, Ordering},
};

use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use http_body_util::BodyExt;
use musicbanana::{
    AppState,
    catalog::Mbids,
    merge::{self, Kind, Options},
    router,
    scrobble::{self, Listen},
    tokens,
};
use serde_json::{Value, json};
use sqlx::PgPool;
use time::{Duration, OffsetDateTime, macros::datetime};
use tower::ServiceExt;
use uuid::Uuid;

// fixtures/profiles.sql: Die Ärzte (artist 1) with Geräusch (release 1) and
// Unrockbar (recording 1), Björk (artist 2) and Tiësto (artist 3), all without IDs.

/// A MusicBrainz ID made up for the tests.
fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

fn artist(n: u128) -> Mbids {
    Mbids {
        artist: Some(id(n)),
        ..Mbids::default()
    }
}

fn album(n: u128) -> Mbids {
    Mbids {
        release_group: Some(id(n)),
        ..Mbids::default()
    }
}

fn track(n: u128) -> Mbids {
    Mbids {
        recording: Some(id(n)),
        ..Mbids::default()
    }
}

const NONE: Mbids = Mbids {
    artist: None,
    release_group: None,
    recording: None,
};

/// An hour apart, so that no listen counts as a duplicate of another.
static HOURS: AtomicI64 = AtomicI64::new(0);

/// Scrobbles a listen to Fiona's default profile and returns the `(artist, album,
/// track)` it counts for.
async fn scrobble(
    db: &PgPool,
    artist: &str,
    title: &str,
    album: Option<&str>,
    mbids: Mbids,
) -> (i64, Option<i64>, i64) {
    let listened_at =
        datetime!(2026-01-01 0:00 UTC) + Duration::hours(HOURS.fetch_add(1, Ordering::Relaxed));
    let listen = Listen {
        listened_at,
        artist: artist.to_owned(),
        track: title.to_owned(),
        album: album.map(str::to_owned),
        track_number: None,
        duration_ms: None,
        client: None,
        mbids,
        extra: None,
    };
    assert_eq!(scrobble::record(db, 1, &[listen]).await.unwrap(), 1);
    sqlx::query_as(
        "SELECT artist_id, release_id, recording_id FROM listen
          WHERE profile_id = 1 AND listened_at = $1",
    )
    .bind(listened_at)
    .fetch_one(db)
    .await
    .unwrap()
}

async fn ids(db: &PgPool, sql: &'static str, entry: i64) -> Vec<Uuid> {
    sqlx::query_scalar(sql)
        .bind(entry)
        .fetch_all(db)
        .await
        .unwrap()
}

async fn artist_ids(db: &PgPool, artist: i64) -> Vec<Uuid> {
    ids(
        db,
        "SELECT mbid FROM artist_mbid WHERE artist_id = $1 ORDER BY mbid",
        artist,
    )
    .await
}

async fn name(db: &PgPool, artist: i64) -> String {
    sqlx::query_scalar("SELECT name FROM artist WHERE id = $1")
        .bind(artist)
        .fetch_one(db)
        .await
        .unwrap()
}

#[sqlx::test(fixtures("profiles"))]
async fn an_id_tells_artists_of_the_same_name_apart(db: PgPool) {
    // The first ID goes to the artist listened to so far …
    let (grunge, _, _) = scrobble(&db, "Nirvana", "Lithium", None, NONE).await;
    assert_eq!(
        scrobble(&db, "Nirvana", "Polly", None, artist(1)).await.0,
        grunge
    );
    // … another one to a new artist of the same name.
    let (sixties, _, rainbow) = scrobble(&db, "Nirvana", "Rainbow Chaser", None, artist(2)).await;
    assert_ne!(sixties, grunge);
    assert_eq!(name(&db, sixties).await, "Nirvana");
    assert_eq!(artist_ids(&db, grunge).await, [id(1)]);
    assert_eq!(artist_ids(&db, sixties).await, [id(2)]);

    // Each ID keeps finding its artist, in any spelling of the name.
    assert_eq!(
        scrobble(&db, "NIRVANA", "Rainbow Chaser", None, artist(2)).await,
        (sixties, None, rainbow)
    );
    assert_eq!(
        scrobble(&db, "nirvana", "Lithium", None, artist(1)).await.0,
        grunge
    );
    // Without an ID, the name leads to the first one.
    assert_eq!(
        scrobble(&db, "Nirvana", "Rainbow Chaser", None, NONE)
            .await
            .0,
        grunge
    );

    // An artist imported without an ID takes the first one too.
    assert_eq!(
        scrobble(&db, "Die Ärzte", "Unrockbar", None, artist(3)).await,
        (1, None, 1)
    );
    assert_eq!(artist_ids(&db, 1).await, [id(3)]);
}

/// Ten listens of `name` at the same time, half of them with the artist ID `one`
/// and half with `other`, as `(IDs, artist)`.
async fn at_once(db: &PgPool, name: &'static str, one: u128, other: u128) -> Vec<(Mbids, i64)> {
    let tasks: Vec<_> = (0..10)
        .map(|i| {
            let db = db.clone();
            let mbids = artist(if i % 2 == 0 { one } else { other });
            tokio::spawn(async move {
                let song = format!("Song {i}");
                (mbids, scrobble(&db, name, &song, None, mbids).await.0)
            })
        })
        .collect();
    let mut found = Vec::new();
    for task in tasks {
        found.push(task.await.unwrap());
    }
    found
}

#[sqlx::test(fixtures("profiles"))]
async fn clients_scrobbling_at_once_agree_on_the_artists(db: PgPool) {
    // Nirvana is new, Weezer was heard before without an ID.
    let heard = scrobble(&db, "Weezer", "Song", None, NONE).await.0;
    for (name, one, other) in [("Nirvana", 1, 2), ("Weezer", 3, 4)] {
        let found = at_once(&db, name, one, other).await;
        // One artist for each ID, with that ID only …
        for (mbids, artist) in &found {
            assert!(
                found.iter().all(|(m, a)| (m == mbids) == (a == artist)),
                "{found:?}"
            );
            assert_eq!(artist_ids(&db, *artist).await, [mbids.artist.unwrap()]);
        }
        // … and the name leads to one of them.
        let named = scrobble(&db, name, "Song", None, NONE).await.0;
        assert!(found.iter().any(|&(_, artist)| artist == named), "{name}");
    }
    assert_eq!(scrobble(&db, "Weezer", "Song", None, NONE).await.0, heard);
}

#[sqlx::test(fixtures("profiles"))]
async fn an_id_is_kept_to_the_name_it_came_with(db: PgPool) {
    // Some files of a duet carry only the ID of its first artist. Neither do the
    // solo listens of that artist go to the duet then …
    let (duet, _, _) = scrobble(&db, "Farin Urlaub & Bela B.", "Duet", None, artist(1)).await;
    let (solo, _, _) = scrobble(&db, "Farin Urlaub", "Solo", None, artist(1)).await;
    assert_ne!(solo, duet);
    assert_eq!(artist_ids(&db, duet).await, [id(1)]);
    assert!(artist_ids(&db, solo).await.is_empty());

    // … nor the other way round.
    let (bela, _, _) = scrobble(&db, "Bela B.", "Solo", None, artist(2)).await;
    let (other_duet, _, _) = scrobble(&db, "Bela B. & Farin Urlaub", "Duet", None, artist(2)).await;
    assert_ne!(other_duet, bela);
    assert_eq!(artist_ids(&db, bela).await, [id(2)]);
    assert_eq!(name(&db, other_duet).await, "Bela B. & Farin Urlaub");
}

#[sqlx::test(fixtures("profiles"))]
async fn an_id_tells_albums_of_the_same_title_apart(db: PgPool) {
    // Geräusch, imported without an ID, takes the first one.
    assert_eq!(
        scrobble(&db, "Die Ärzte", "Unrockbar", Some("Geräusch"), album(10)).await,
        (1, Some(1), 1)
    );
    // Another album of the same title, e.g. a single named like the album it is from.
    let (_, single, _) = scrobble(&db, "Die Ärzte", "Unrockbar", Some("Geräusch"), album(11)).await;
    assert_ne!(single, Some(1));
    assert_eq!(
        scrobble(
            &db,
            "Die Ärzte",
            "Deine Schuld",
            Some("GERÄUSCH"),
            album(11)
        )
        .await
        .1,
        single
    );
    // Without an ID, the title leads to the first one.
    assert_eq!(
        scrobble(&db, "Die Ärzte", "Deine Schuld", Some("Geräusch"), NONE)
            .await
            .1,
        Some(1)
    );

    // An edition under another title is the same album, and so is that title
    // without an ID from now on.
    assert_eq!(
        scrobble(
            &db,
            "Die Ärzte",
            "Unrockbar",
            Some("Geräusch (Deluxe)"),
            album(10)
        )
        .await
        .1,
        Some(1)
    );
    assert_eq!(
        scrobble(
            &db,
            "Die Ärzte",
            "Unrockbar",
            Some("Geräusch (Deluxe)"),
            NONE
        )
        .await
        .1,
        Some(1)
    );

    // A compilation is an album of each artist on it.
    let (_, of_björk, _) = scrobble(&db, "Björk", "Jóga", Some("Geräusch"), album(10)).await;
    assert_ne!(of_björk, Some(1));
}

async fn recording_ids(db: &PgPool, recording: i64) -> Vec<Uuid> {
    ids(
        db,
        "SELECT mbid FROM recording_mbid WHERE recording_id = $1 ORDER BY mbid",
        recording,
    )
    .await
}

#[sqlx::test(fixtures("profiles"))]
async fn an_id_tells_tracks_of_the_same_title_apart(db: PgPool) {
    // Unrockbar, imported without an ID, takes the first one …
    assert_eq!(
        scrobble(&db, "Die Ärzte", "Unrockbar", Some("Geräusch"), track(20)).await,
        (1, Some(1), 1)
    );
    // … a live version of it is another track of the same title …
    let (_, _, live) = scrobble(&db, "Die Ärzte", "Unrockbar", Some("Live"), track(21)).await;
    assert_ne!(live, 1);
    // … and the same recording on another album, here remastered, is the first.
    assert_eq!(
        scrobble(
            &db,
            "Die Ärzte",
            "Unrockbar (Remastered)",
            Some("Best of"),
            track(20)
        )
        .await
        .2,
        1
    );
    assert_eq!(recording_ids(&db, 1).await, [id(20)]);
    assert_eq!(recording_ids(&db, live).await, [id(21)]);

    // Without an ID, the titles lead to the first one.
    for title in ["Unrockbar", "Unrockbar (Remastered)"] {
        assert_eq!(
            scrobble(&db, "Die Ärzte", title, Some("Live"), NONE)
                .await
                .2,
            1,
            "{title}"
        );
    }
    assert_eq!(
        scrobble(&db, "Die Ärzte", "Unrockbar", None, track(21))
            .await
            .2,
        live
    );

    // They are not suggested for merging, and merging them takes --force.
    assert!(
        merge::suggest(&db, Kind::Recording)
            .await
            .unwrap()
            .is_empty()
    );
    let refused = merge::merge(&db, Kind::Recording, live, 1, Options::default())
        .await
        .unwrap_err();
    assert!(
        refused.to_string().contains("different MusicBrainz IDs"),
        "{refused}"
    );
}

#[sqlx::test(fixtures("profiles"))]
async fn merging_respects_and_keeps_the_ids(db: PgPool) {
    let (grunge, _, _) = scrobble(&db, "Nirvana", "Lithium", None, artist(1)).await;
    let (sixties, _, _) = scrobble(&db, "Nirvana", "Rainbow Chaser", None, artist(2)).await;
    let (typo, _, _) = scrobble(&db, "Nirvanna", "Polly", None, NONE).await;

    // Of the look-alikes, only the misspelling is suggested, not the other band.
    let suggested: Vec<(i64, i64)> = merge::suggest(&db, Kind::Artist)
        .await
        .unwrap()
        .into_iter()
        .map(|s| (s.from.id, s.into.id))
        .collect();
    assert_eq!(suggested, [(typo, grunge)]);
    let refused = merge::merge(&db, Kind::Artist, sixties, grunge, Options::default())
        .await
        .unwrap_err();
    assert_eq!(
        refused.to_string(),
        format!(
            "artist {sixties} and {grunge} have different MusicBrainz IDs, so they are not \
             the same; --force merges them anyway"
        )
    );

    // An artist's ID goes along when it is merged into another spelling.
    let (aerzte, _, _) = scrobble(&db, "Die Aerzte", "Schrei nach Liebe", None, artist(3)).await;
    assert_ne!(aerzte, 1);
    merge::merge(&db, Kind::Artist, aerzte, 1, Options::default())
        .await
        .unwrap();
    assert_eq!(artist_ids(&db, 1).await, [id(3)]);
    for spelling in ["Die Ärzte", "Die Aerzte"] {
        assert_eq!(
            scrobble(&db, spelling, "Schrei nach Liebe", None, artist(3))
                .await
                .0,
            1,
            "{spelling}"
        );
    }
}

#[sqlx::test(fixtures("profiles"))]
async fn merging_artists_merges_the_albums_with_the_same_id(db: PgPool) {
    let with = |release_group| Mbids {
        artist: Some(id(1)),
        release_group: Some(id(release_group)),
        recording: None,
    };
    // A band with two albums of the same title …
    let (band, blue, _) = scrobble(&db, "Weezer", "One", Some("Weezer"), with(10)).await;
    let (_, green, _) = scrobble(&db, "Weezer", "Two", Some("Weezer"), with(11)).await;
    assert_ne!(green, blue);
    // … and a misspelling of it, without an artist ID, with one of them, a third
    // album of that title and an album without an ID.
    let (typo, typo_green, _) = scrobble(&db, "Weezr", "Three", Some("Weezer"), album(11)).await;
    let (_, red, _) = scrobble(&db, "Weezr", "Four", Some("Weezer"), album(12)).await;
    let (_, pinkerton, _) = scrobble(&db, "Weezr", "Five", Some("Pinkerton"), NONE).await;
    assert_ne!(red, typo_green);

    let merged = merge::merge(&db, Kind::Artist, typo, band, Options::default())
        .await
        .unwrap();
    assert_eq!((merged.releases_moved, merged.releases_merged), (2, 1));
    let merged_into: Option<i64> =
        sqlx::query_scalar("SELECT merged_into FROM release WHERE id = $1")
            .bind(typo_green)
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(merged_into, green);

    // Each album ID still finds its album, and the title alone the first one.
    for (mbids, album) in [
        (with(10), blue),
        (with(11), green),
        (with(12), red),
        (NONE, blue),
    ] {
        let (artist, release, _) = scrobble(&db, "Weezer", "Six", Some("Weezer"), mbids).await;
        assert_eq!((artist, release), (band, album), "{mbids:?}");
    }
    assert_eq!(
        scrobble(&db, "Weezer", "Seven", Some("Pinkerton"), NONE)
            .await
            .1,
        pinkerton
    );
}

#[sqlx::test(fixtures("profiles"))]
async fn merging_artists_merges_the_tracks_with_the_same_id(db: PgPool) {
    let with = |recording| Mbids {
        artist: Some(id(1)),
        recording: Some(id(recording)),
        ..Mbids::default()
    };
    // A band with a song and a live version of it …
    let (band, _, studio) = scrobble(&db, "Weezer", "Buddy Holly", None, with(20)).await;
    let (_, _, live) = scrobble(&db, "Weezer", "Buddy Holly", Some("Live"), with(21)).await;
    assert_ne!(live, studio);
    // … and a misspelling of the band without an artist ID, with that live
    // version, another live version and a song without an ID.
    let (typo, _, typo_live) = scrobble(&db, "Weezr", "Buddy Holly", None, track(21)).await;
    let (_, _, other_live) = scrobble(&db, "Weezr", "Buddy Holly", None, track(22)).await;
    let (_, _, plain) = scrobble(&db, "Weezr", "Say It Ain't So", None, NONE).await;
    assert_ne!(other_live, typo_live);

    let merged = merge::merge(&db, Kind::Artist, typo, band, Options::default())
        .await
        .unwrap();
    assert_eq!((merged.recordings_moved, merged.recordings_merged), (2, 1));

    // Each recording ID still finds its track, and the title alone the first one.
    for (mbids, track) in [
        (with(20), studio),
        (with(21), live),
        (with(22), other_live),
        (NONE, studio),
    ] {
        let (artist, _, recording) = scrobble(&db, "Weezer", "Buddy Holly", None, mbids).await;
        assert_eq!((artist, recording), (band, track), "{mbids:?}");
    }
    assert_eq!(
        scrobble(&db, "Weezer", "Say It Ain't So", None, NONE)
            .await
            .2,
        plain
    );
}

#[sqlx::test(fixtures("profiles"))]
async fn a_forced_merge_joins_what_the_ids_tell_apart(db: PgPool) {
    let force = Options {
        force: true,
        ..Options::default()
    };
    // An artist's band, which MusicBrainz lists as an artist of its own …
    let (farin, _, _) = scrobble(&db, "Farin Urlaub", "Sumisu", None, artist(1)).await;
    let (band, _, _) = scrobble(
        &db,
        "Farin Urlaub Racing Team",
        "Porzellan",
        None,
        artist(2),
    )
    .await;
    merge::merge(&db, Kind::Artist, band, farin, force)
        .await
        .unwrap();
    // … counts for the artist from then on, with its ID or without.
    assert_eq!(artist_ids(&db, farin).await, [id(1), id(2)]);
    for mbids in [artist(2), NONE] {
        assert_eq!(
            scrobble(&db, "Farin Urlaub Racing Team", "Porzellan", None, mbids)
                .await
                .0,
            farin,
            "{mbids:?}"
        );
    }

    // The same goes for a recording that MusicBrainz lists twice.
    assert_eq!(
        scrobble(&db, "Die Ärzte", "Unrockbar", Some("Geräusch"), track(20))
            .await
            .2,
        1
    );
    let (_, _, again) = scrobble(&db, "Die Ärzte", "Unrockbar", Some("Best of"), track(21)).await;
    assert_ne!(again, 1);
    merge::merge(&db, Kind::Recording, again, 1, force)
        .await
        .unwrap();
    assert_eq!(recording_ids(&db, 1).await, [id(20), id(21)]);
    assert_eq!(
        scrobble(&db, "Die Ärzte", "Unrockbar", Some("Best of"), track(21))
            .await
            .2,
        1
    );
}

/// GET or POST against the API, as the frontend and Navidrome do.
async fn send(db: &PgPool, request: Request<Body>) -> (StatusCode, Value) {
    let app = router(AppState::new(db.clone()), Path::new("does-not-exist"));
    let res = app.oneshot(request).await.unwrap();
    let status = res.status();
    let body = res.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&body).unwrap_or(Value::Null))
}

#[sqlx::test(fixtures("profiles"))]
async fn the_ids_navidrome_sends_show_on_the_pages(db: PgPool) {
    let token = tokens::create(&db, "fiona", "default", "Navidrome")
        .await
        .unwrap()
        .token;
    let listen = json!({
        "listen_type": "single",
        "payload": [{
            "listened_at": OffsetDateTime::now_utc().unix_timestamp() - 60,
            "track_metadata": {
                "artist_name": "Die Ärzte",
                "track_name": "Unrockbar",
                "release_name": "Geräusch",
                "additional_info": {
                    "submission_client": "Navidrome",
                    "artist_names": ["Die Ärzte"],
                    "artist_mbids": [id(1)],
                    "recording_mbid": id(3),
                    "release_mbid": id(4),
                    "release_group_mbid": id(2),
                },
            },
        }],
    });
    let request = Request::post("/api/listenbrainz/1/submit-listens")
        .header(header::AUTHORIZATION, format!("Token {token}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(listen.to_string()))
        .unwrap();
    assert_eq!(send(&db, request).await.0, StatusCode::OK);

    for (page, mbid) in [
        ("artists", id(1)),
        ("releases", id(2)),
        ("recordings", id(3)),
    ] {
        let uri = format!("/api/profiles/fiona/default/{page}/1");
        let (status, body) = send(&db, Request::get(&uri).body(Body::empty()).unwrap()).await;
        assert_eq!(status, StatusCode::OK, "{uri}");
        assert_eq!(body["mbids"], json!([mbid]), "{uri}");
    }
    let (_, björk) = send(
        &db,
        Request::get("/api/profiles/fiona/default/artists/2")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(björk["mbids"], json!([]));
}
