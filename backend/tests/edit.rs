//! Renaming catalog entries and giving them MusicBrainz IDs by hand.

use std::sync::atomic::{AtomicI64, Ordering};

use musicbanana::{
    catalog::Mbids,
    edit::{self, Named},
    merge::{self, Kind, Options},
    scrobble::{self, Listen},
};
use sqlx::PgPool;
use time::{Duration, macros::datetime};
use uuid::Uuid;

// fixtures/profiles.sql: Die Ärzte (artist 1) with Geräusch (release 1) and
// Unrockbar (recording 1), Björk (artist 2) and Tiësto (artist 3), all without IDs.

/// A MusicBrainz ID made up for the tests.
fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

const NONE: Mbids = Mbids {
    artist: None,
    release_group: None,
    recording: None,
};

fn artist(n: u128) -> Mbids {
    Mbids {
        artist: Some(id(n)),
        ..NONE
    }
}

fn track(n: u128) -> Mbids {
    Mbids {
        recording: Some(id(n)),
        ..NONE
    }
}

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

fn error<T: std::fmt::Debug>(result: anyhow::Result<T>) -> String {
    result.unwrap_err().to_string()
}

async fn name(db: &PgPool, sql: &'static str, entry: i64) -> String {
    sqlx::query_scalar(sql)
        .bind(entry)
        .fetch_one(db)
        .await
        .unwrap()
}

#[sqlx::test(fixtures("profiles"))]
async fn a_renamed_entry_keeps_its_old_spellings(db: PgPool) {
    let renamed = edit::rename(&db, Kind::Artist, 1, " Die Ärzte (Berlin) ")
        .await
        .unwrap();
    assert_eq!(
        renamed.to_string(),
        "Renamed artist 1 \"Die Ärzte\" to \"Die Ärzte (Berlin)\"."
    );
    assert_eq!(
        name(&db, "SELECT name FROM artist WHERE id = $1", 1).await,
        "Die Ärzte (Berlin)"
    );
    edit::rename(&db, Kind::Release, 1, "Geräusch (2003)")
        .await
        .unwrap();
    edit::rename(&db, Kind::Recording, 1, "Unrockbar!")
        .await
        .unwrap();
    assert_eq!(
        name(&db, "SELECT title FROM recording WHERE id = $1", 1).await,
        "Unrockbar!"
    );

    // Old and new names lead to the entries.
    for (artist, title, album) in [
        ("Die Ärzte", "Unrockbar", "Geräusch"),
        ("die ärzte (berlin)", "UNROCKBAR!", "Geräusch (2003)"),
    ] {
        assert_eq!(
            scrobble(&db, artist, title, Some(album), NONE).await,
            (1, Some(1), 1),
            "{artist}"
        );
    }
}

#[sqlx::test(fixtures("profiles"))]
async fn renaming_tells_artists_of_the_same_name_apart(db: PgPool) {
    let (grunge, _, _) = scrobble(&db, "Nirvana", "Lithium", None, artist(1)).await;
    let (sixties, _, _) = scrobble(&db, "Nirvana", "Rainbow Chaser", None, artist(2)).await;
    assert_ne!(sixties, grunge);
    edit::rename(&db, Kind::Artist, sixties, "Nirvana (UK)")
        .await
        .unwrap();

    // The ID keeps counting under the name it came with, which leads to the
    // other Nirvana without an ID, and the new name leads to the renamed one.
    for (name, mbids, artist) in [
        ("Nirvana", artist(2), sixties),
        ("Nirvana", NONE, grunge),
        ("Nirvana", artist(1), grunge),
        ("Nirvana (UK)", artist(2), sixties),
        ("Nirvana (UK)", NONE, sixties),
    ] {
        assert_eq!(
            scrobble(&db, name, "Song", None, mbids).await.0,
            artist,
            "{name} {mbids:?}"
        );
    }
}

#[sqlx::test(fixtures("profiles"))]
async fn a_name_of_another_entry_stays_with_that_one(db: PgPool) {
    let renamed = edit::rename(&db, Kind::Artist, 2, "Tiësto").await.unwrap();
    assert_eq!(
        renamed.taken_by,
        Some(Named {
            kind: Kind::Artist,
            id: 3,
            name: "Tiësto".to_owned()
        })
    );
    assert_eq!(
        renamed.to_string(),
        "Renamed artist 2 \"Björk\" to \"Tiësto\".\nListens named \"Tiësto\" without a \
         MusicBrainz ID keep counting for artist 3 \"Tiësto\". If the two are the same: \
         musicbanana merge artist 2 3"
    );
    assert_eq!(scrobble(&db, "Tiësto", "Song", None, NONE).await.0, 3);
    assert_eq!(scrobble(&db, "Björk", "Song", None, NONE).await.0, 2);

    // A title is a spelling per artist.
    let renamed = edit::rename(&db, Kind::Recording, 1, "Deine Schuld")
        .await
        .unwrap();
    assert_eq!(renamed.taken_by.map(|other| other.id), Some(2));
    let renamed = edit::rename(&db, Kind::Recording, 3, "Deine Schuld")
        .await
        .unwrap();
    assert_eq!(renamed.taken_by, None);
}

#[sqlx::test(fixtures("profiles"))]
async fn refuses_impossible_changes(db: PgPool) {
    assert_eq!(
        error(edit::rename(&db, Kind::Release, 99, "X").await),
        "there is no release 99"
    );
    assert_eq!(
        error(edit::rename(&db, Kind::Artist, 1, "  ").await),
        "the new name is empty"
    );
    merge::merge(&db, Kind::Artist, 3, 2, Options::default())
        .await
        .unwrap();
    assert_eq!(
        error(edit::add_mbid(&db, Kind::Artist, 3, id(1)).await),
        "artist 3 was merged into 2; change that one instead"
    );

    assert_eq!(
        error(edit::remove_mbid(&db, Kind::Artist, 1, id(1)).await),
        "artist 1 \"Die Ärzte\" has no MusicBrainz ID"
    );
    edit::add_mbid(&db, Kind::Artist, 1, id(1)).await.unwrap();
    assert_eq!(
        error(edit::remove_mbid(&db, Kind::Artist, 1, id(2)).await),
        format!(
            "artist 1 \"Die Ärzte\" does not have the MusicBrainz ID {}, only {}",
            id(2),
            id(1)
        )
    );
    assert_eq!(
        error(edit::add_mbid(&db, Kind::Artist, 2, id(1)).await),
        format!(
            "the MusicBrainz ID {} belongs to artist 1; take it from that one first, or \
             merge the two",
            id(1)
        )
    );
}

#[sqlx::test(fixtures("profiles"))]
async fn an_id_given_by_hand_finds_its_entry(db: PgPool) {
    let (entry, new) = edit::add_mbid(&db, Kind::Artist, 1, id(1)).await.unwrap();
    assert_eq!(entry.to_string(), "artist 1 \"Die Ärzte\"");
    assert!(new);
    assert!(!edit::add_mbid(&db, Kind::Artist, 1, id(1)).await.unwrap().1);
    edit::add_mbid(&db, Kind::Release, 1, id(10)).await.unwrap();
    edit::add_mbid(&db, Kind::Recording, 1, id(20))
        .await
        .unwrap();

    let ids = Mbids {
        artist: Some(id(1)),
        release_group: Some(id(10)),
        recording: Some(id(20)),
    };
    assert_eq!(
        scrobble(
            &db,
            "Die Ärzte",
            "Unrockbar (Remastered)",
            Some("Geräusch (Deluxe)"),
            ids
        )
        .await,
        (1, Some(1), 1)
    );
    // Another album and track of the same title are new ones now.
    let (_, single, live) = scrobble(
        &db,
        "Die Ärzte",
        "Unrockbar",
        Some("Geräusch"),
        Mbids {
            release_group: Some(id(11)),
            recording: Some(id(21)),
            ..NONE
        },
    )
    .await;
    assert_ne!(single, Some(1));
    assert_ne!(live, 1);
}

#[sqlx::test(fixtures("profiles"))]
async fn an_id_that_came_to_the_wrong_entry_can_be_moved(db: PgPool) {
    // The imported Unrockbar took the ID of a live version, as that came first …
    assert_eq!(
        scrobble(&db, "Die Ärzte", "Unrockbar", Some("Live"), track(21))
            .await
            .2,
        1
    );
    let (_, _, studio) = scrobble(&db, "Die Ärzte", "Unrockbar", Some("Geräusch"), track(20)).await;
    assert_ne!(studio, 1);
    // … so it gives that ID up and the studio version goes into it …
    let entry = edit::remove_mbid(&db, Kind::Recording, 1, id(21))
        .await
        .unwrap();
    assert_eq!(entry.to_string(), "recording 1 \"Unrockbar\"");
    merge::merge(&db, Kind::Recording, studio, 1, Options::default())
        .await
        .unwrap();
    assert_eq!(
        scrobble(&db, "Die Ärzte", "Unrockbar", Some("Geräusch"), track(20))
            .await
            .2,
        1
    );
    // … and the live version gets a track of its own from then on.
    let (_, _, live) = scrobble(&db, "Die Ärzte", "Unrockbar", Some("Live"), track(21)).await;
    assert_ne!(live, 1);

    // Likewise for an artist ID: taken from Björk, it goes to Tiësto by name.
    edit::add_mbid(&db, Kind::Artist, 2, id(3)).await.unwrap();
    edit::remove_mbid(&db, Kind::Artist, 2, id(3))
        .await
        .unwrap();
    assert_eq!(scrobble(&db, "Tiësto", "Song", None, artist(3)).await.0, 3);
}
