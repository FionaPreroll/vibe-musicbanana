//! Merging duplicates in the catalog, and the suggestions for it.

use musicbanana::{
    merge::{self, Kind, Likeness},
    scrobble::{self, Listen},
};
use sqlx::PgPool;
use time::OffsetDateTime;

// fixtures/profiles.sql has Die Ärzte, Björk and Tiësto with their releases and
// recordings; fixtures/merge.sql adds the look-alikes "Die Aerzte" (ids: artist 4,
// releases 3 and 4, recordings 6 and 7), "Bjork" (artist 5, recording 9) and
// "Unrockbar (Live)" (recording 8).

/// `(from, into, likeness)` of every suggestion.
async fn suggested(db: &PgPool, kind: Kind) -> Vec<(i64, i64, Likeness)> {
    merge::suggest(db, kind)
        .await
        .unwrap()
        .into_iter()
        .map(|s| (s.from.id, s.into.id, s.likeness))
        .collect()
}

/// `(artist, recording, release)` of the listen at `at`.
async fn catalog_of(db: &PgPool, at: &str) -> (i64, i64, Option<i64>) {
    sqlx::query_as(
        "SELECT artist_id, recording_id, release_id FROM listen WHERE listened_at = $1::text::timestamptz",
    )
    .bind(at)
    .fetch_one(db)
    .await
    .unwrap()
}

async fn count(db: &PgPool, sql: &'static str) -> i64 {
    sqlx::query_scalar(sql).fetch_one(db).await.unwrap()
}

/// A new listen in Fiona's default profile.
async fn scrobble(db: &PgPool, at: &str, artist: &str, track: &str, album: Option<&str>) {
    let listen = Listen {
        listened_at: OffsetDateTime::parse(at, &time::format_description::well_known::Rfc3339)
            .unwrap(),
        artist: artist.to_owned(),
        track: track.to_owned(),
        album: album.map(str::to_owned),
        track_number: None,
        duration_ms: None,
        client: None,
        extra: None,
    };
    assert_eq!(scrobble::record(db, 1, &[listen]).await.unwrap(), 1);
}

#[sqlx::test(fixtures("profiles", "merge"))]
async fn suggests_look_alikes_into_the_most_listened(db: PgPool) {
    assert_eq!(
        suggested(&db, Kind::Artist).await,
        [(5, 2, Likeness::SameLetters), (4, 1, Likeness::OneLetter)]
    );
    // Only within an artist: the two Geräusch are by different artists so far.
    assert_eq!(suggested(&db, Kind::Release).await, []);
    assert_eq!(
        suggested(&db, Kind::Recording).await,
        [(8, 1, Likeness::Version)]
    );

    let found = merge::suggest(&db, Kind::Recording).await.unwrap();
    assert_eq!(found[0].from.name, "Unrockbar (Live)");
    assert_eq!(found[0].into.artist.as_deref(), Some("Die Ärzte"));
    assert_eq!((found[0].from.listens, found[0].into.listens), (1, 3));
}

#[sqlx::test(fixtures("profiles", "merge"))]
async fn merging_an_artist_takes_its_releases_recordings_and_spellings_along(db: PgPool) {
    let catalog = "SELECT (SELECT count(*) FROM artist) + (SELECT count(*) FROM release)
                        + (SELECT count(*) FROM recording)";
    let entries = count(&db, catalog).await;

    let merged = merge::merge(&db, Kind::Artist, 4, 1, false).await.unwrap();
    assert_eq!((merged.listens, merged.spellings), (2, 1));
    assert_eq!((merged.releases_moved, merged.releases_merged), (1, 1));
    assert_eq!((merged.recordings_moved, merged.recordings_merged), (1, 1));
    assert_eq!(merged.from, (4, "Die Aerzte".to_owned()));
    assert_eq!(merged.into, (1, "Die Ärzte".to_owned()));

    // Geräusch and Unrockbar went into Die Ärzte's, the others moved over.
    assert_eq!(
        catalog_of(&db, "2016-05-01T12:00:00Z").await,
        (1, 1, Some(1))
    );
    assert_eq!(
        catalog_of(&db, "2016-05-02T12:00:00Z").await,
        (1, 7, Some(4))
    );
    // (artist, merged into) of the old releases 3 and 4, then recordings 6 and 7.
    let owners: Vec<(i64, Option<i64>)> = sqlx::query_as(
        "(SELECT artist_id, merged_into FROM release WHERE id IN (3, 4) ORDER BY id)
         UNION ALL
         (SELECT artist_id, merged_into FROM recording WHERE id IN (6, 7) ORDER BY id)",
    )
    .fetch_all(&db)
    .await
    .unwrap();
    assert_eq!(owners, [(4, Some(1)), (1, None), (4, Some(1)), (1, None)]);
    let merged_into: Option<i64> =
        sqlx::query_scalar("SELECT merged_into FROM artist WHERE id = 4")
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(merged_into, Some(1));
    // What only the merged recording knew stays.
    let length: Option<i32> = sqlx::query_scalar("SELECT length_ms FROM recording WHERE id = 1")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(length, Some(222000));
    // No spelling leads to the merged artist any more.
    assert_eq!(
        count(
            &db,
            "SELECT (SELECT count(*) FROM artist_alias WHERE artist_id = 4)
                  + (SELECT count(*) FROM release_alias WHERE artist_id = 4)
                  + (SELECT count(*) FROM recording_alias WHERE artist_id = 4)"
        )
        .await,
        0
    );

    // New scrobbles with the old spelling land on the merged entries.
    scrobble(
        &db,
        "2016-06-01T12:00:00Z",
        "DIE AERZTE",
        "Schrei nach Liebe",
        Some("Jazz ist anders"),
    )
    .await;
    scrobble(
        &db,
        "2016-06-02T12:00:00Z",
        "Die Aerzte",
        "Unrockbar",
        Some("Geräusch"),
    )
    .await;
    assert_eq!(
        catalog_of(&db, "2016-06-01T12:00:00Z").await,
        (1, 7, Some(4))
    );
    assert_eq!(
        catalog_of(&db, "2016-06-02T12:00:00Z").await,
        (1, 1, Some(1))
    );
    assert_eq!(count(&db, catalog).await, entries);

    // Now one artist, whose live version is still a suggestion.
    assert_eq!(
        suggested(&db, Kind::Artist).await,
        [(5, 2, Likeness::SameLetters)]
    );
    assert_eq!(
        suggested(&db, Kind::Recording).await,
        [(8, 1, Likeness::Version)]
    );
}

#[sqlx::test(fixtures("profiles", "merge"))]
async fn a_dry_run_changes_nothing(db: PgPool) {
    let merged = merge::merge(&db, Kind::Artist, 5, 2, true).await.unwrap();
    assert!(merged.dry_run);
    assert_eq!((merged.listens, merged.spellings), (1, 1));
    // "Joga" is not "Jóga", so it would move over.
    assert_eq!((merged.recordings_moved, merged.recordings_merged), (1, 0));

    assert_eq!(catalog_of(&db, "2016-05-04T12:00:00Z").await, (5, 9, None));
    assert_eq!(
        count(
            &db,
            "SELECT count(*) FROM artist WHERE merged_into IS NOT NULL"
        )
        .await,
        0
    );
}

#[sqlx::test(fixtures("profiles", "merge"))]
async fn merges_recordings_and_releases(db: PgPool) {
    merge::merge(&db, Kind::Artist, 5, 2, false).await.unwrap();
    // Björk now has "Jóga" and "Joga"; the older entry wins the tie.
    assert_eq!(
        suggested(&db, Kind::Recording).await,
        [(9, 4, Likeness::SameLetters), (8, 1, Likeness::Version)]
    );
    let merged = merge::merge(&db, Kind::Recording, 9, 4, false)
        .await
        .unwrap();
    assert_eq!((merged.listens, merged.spellings), (1, 1));
    scrobble(&db, "2016-06-01T12:00:00Z", "Bjork", "Joga", None).await;
    assert_eq!(catalog_of(&db, "2016-06-01T12:00:00Z").await, (2, 4, None));

    // A release merges across artists too: "Die Aerzte" stays, its Geräusch goes.
    let merged = merge::merge(&db, Kind::Release, 3, 1, false).await.unwrap();
    assert_eq!((merged.listens, merged.spellings), (1, 1));
    scrobble(
        &db,
        "2016-06-02T12:00:00Z",
        "Die Aerzte",
        "Unrockbar",
        Some("Geräusch"),
    )
    .await;
    assert_eq!(
        catalog_of(&db, "2016-06-02T12:00:00Z").await,
        (4, 6, Some(1))
    );
}

#[sqlx::test(fixtures("profiles", "merge"))]
async fn refuses_impossible_merges(db: PgPool) {
    let error = |kind, from, into| {
        let db = db.clone();
        async move {
            merge::merge(&db, kind, from, into, false)
                .await
                .unwrap_err()
                .to_string()
        }
    };
    assert_eq!(
        error(Kind::Artist, 1, 1).await,
        "artist 1 cannot be merged into itself"
    );
    assert_eq!(error(Kind::Release, 99, 1).await, "there is no release 99");

    merge::merge(&db, Kind::Artist, 4, 1, false).await.unwrap();
    assert_eq!(
        error(Kind::Artist, 4, 1).await,
        "artist 4 was merged into 1 already"
    );
    assert_eq!(
        error(Kind::Artist, 2, 4).await,
        "artist 4 was merged into 1; merge into that one instead"
    );

    sqlx::query("UPDATE artist SET mbid = gen_random_uuid() WHERE id IN (2, 5)")
        .execute(&db)
        .await
        .unwrap();
    assert_eq!(
        error(Kind::Artist, 5, 2).await,
        "artist 5 and 2 have different MusicBrainz IDs, so they are not the same"
    );
}
