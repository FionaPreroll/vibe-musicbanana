//! Edition notes such as "- Remastered 2011" or "(Deluxe Edition)": left out of
//! new listens, and taken off the catalog afterwards by `merge editions`.

use musicbanana::{
    merge::{self, Edition},
    scrobble::{self, Listen},
};
use sqlx::PgPool;
use time::OffsetDateTime;

/// A new listen in Fiona's default profile; returns its `(recording, release)`.
async fn scrobble(
    db: &PgPool,
    at: &str,
    artist: &str,
    track: &str,
    album: Option<&str>,
) -> (i64, Option<i64>) {
    let listened_at =
        OffsetDateTime::parse(at, &time::format_description::well_known::Rfc3339).unwrap();
    let listen = Listen {
        listened_at,
        artist: artist.to_owned(),
        track: track.to_owned(),
        album: album.map(str::to_owned),
        track_number: None,
        duration_ms: None,
        client: None,
        mbids: Default::default(),
        extra: None,
    };
    assert_eq!(scrobble::record(db, 1, &[listen]).await.unwrap(), 1);
    sqlx::query_as("SELECT recording_id, release_id FROM listen WHERE listened_at = $1")
        .bind(listened_at)
        .fetch_one(db)
        .await
        .unwrap()
}

async fn title(db: &PgPool, table: &str, id: i64) -> (String, Option<i64>) {
    let sql = match table {
        "release" => "SELECT title, merged_into FROM release WHERE id = $1",
        _ => "SELECT title, merged_into FROM recording WHERE id = $1",
    };
    sqlx::query_as(sql).bind(id).fetch_one(db).await.unwrap()
}

/// The lines `merge editions` prints.
fn lines(done: &[Edition]) -> Vec<String> {
    done.iter().map(ToString::to_string).collect()
}

#[sqlx::test(fixtures("profiles"))]
async fn new_listens_leave_edition_notes_out(db: PgPool) {
    // Unrockbar from Geräusch, as Spotify names a remaster and a deluxe edition.
    let noted = scrobble(
        &db,
        "2024-01-01T12:00:00Z",
        "Die Ärzte",
        "Unrockbar - Remastered 2011",
        Some("Geräusch (Deluxe Edition)"),
    )
    .await;
    assert_eq!(noted, (1, Some(1)));

    // A track heard first with a note gets the plain title, which then finds it.
    let first = scrobble(
        &db,
        "2024-01-02T12:00:00Z",
        "Die Ärzte",
        "Schrei nach Liebe - 2012 Remaster",
        Some("Die Bestie in Menschengestalt [Remastered]"),
    )
    .await;
    assert_eq!(
        title(&db, "recording", first.0).await.0,
        "Schrei nach Liebe"
    );
    assert_eq!(
        title(&db, "release", first.1.unwrap()).await.0,
        "Die Bestie in Menschengestalt"
    );
    let plain = scrobble(
        &db,
        "2024-01-03T12:00:00Z",
        "Die Ärzte",
        "Schrei nach Liebe",
        Some("Die Bestie in Menschengestalt"),
    )
    .await;
    assert_eq!(plain, first);

    // Other versions stay apart, and the listens keep what was sent.
    let live = scrobble(
        &db,
        "2024-01-04T12:00:00Z",
        "Die Ärzte",
        "Unrockbar (Live)",
        None,
    )
    .await;
    assert_ne!(live.0, 1);
    let raw: (String, Option<String>) = sqlx::query_as(
        "SELECT track_raw, album_raw FROM listen WHERE listened_at = '2024-01-01T12:00:00Z'",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(
        raw,
        (
            "Unrockbar - Remastered 2011".to_owned(),
            Some("Geräusch (Deluxe Edition)".to_owned())
        )
    );
}

#[sqlx::test(fixtures("profiles", "editions"))]
async fn merge_editions_cleans_up_what_came_before(db: PgPool) {
    let expected = [
        r#"merge release 10 "Geräusch (Deluxe Edition)" into 1 "Geräusch": 2 listens"#,
        r#"merge recording 10 "Unrockbar - Remastered 2011" into 1 "Unrockbar": 1 listen"#,
        r#"keep recording 11 "Deine Schuld - 2012 Remaster": 2 "Deine Schuld" has other MusicBrainz IDs"#,
        r#"rename recording 13 "Hyperballad - Remastered" to "Hyperballad""#,
        r#"merge recording 14 "Hyperballad (2015 Remaster)" into 13 "Hyperballad": 1 listen"#,
    ];

    // A dry run tells the same and changes nothing.
    let listens = "SELECT array_agg(recording_id ORDER BY listened_at) FROM listen";
    let before: Vec<i64> = sqlx::query_scalar(listens).fetch_one(&db).await.unwrap();
    assert_eq!(lines(&merge::editions(&db, true).await.unwrap()), expected);
    assert_eq!(
        sqlx::query_scalar::<_, Vec<i64>>(listens)
            .fetch_one(&db)
            .await
            .unwrap(),
        before
    );
    assert_eq!(
        title(&db, "recording", 13).await.0,
        "Hyperballad - Remastered"
    );

    // For real, each merge with its number for `merge undo`.
    let done = merge::editions(&db, false).await.unwrap();
    let ops: Vec<i64> = done
        .iter()
        .filter_map(|e| match e {
            Edition::Merged(m) => m.op,
            _ => None,
        })
        .collect();
    assert_eq!(ops.len(), 3);
    let mut numbered = ops.iter();
    let expected_done: Vec<String> = expected
        .iter()
        .map(|line| match line.starts_with("merge") {
            true => format!(
                "{line} (undo with: musicbanana merge undo {})",
                numbered.next().unwrap()
            ),
            false => line.to_string(),
        })
        .collect();
    assert_eq!(lines(&done), expected_done);
    assert_eq!(title(&db, "release", 10).await.1, Some(1));
    assert_eq!(title(&db, "recording", 10).await.1, Some(1));
    assert_eq!(title(&db, "recording", 11).await.1, None);
    assert_eq!(
        title(&db, "recording", 12).await,
        ("Human Behaviour (Live)".to_owned(), None)
    );
    assert_eq!(
        title(&db, "recording", 13).await,
        ("Hyperballad".to_owned(), None)
    );
    assert_eq!(title(&db, "recording", 14).await.1, Some(13));

    // Every spelling leads to what is left, and a second run only finds the one kept apart.
    let at = |n: u32| format!("2024-02-{n:02}T12:00:00Z");
    assert_eq!(
        scrobble(&db, &at(1), "Björk", "Hyperballad", None).await.0,
        13
    );
    assert_eq!(
        scrobble(&db, &at(2), "Björk", "Hyperballad (2015 Remaster)", None)
            .await
            .0,
        13
    );
    assert_eq!(
        scrobble(
            &db,
            &at(3),
            "Die Ärzte",
            "Unrockbar",
            Some("Geräusch (Deluxe Edition)")
        )
        .await,
        (1, Some(1))
    );
    assert_eq!(
        lines(&merge::editions(&db, false).await.unwrap()),
        [expected[2]]
    );

    // The last merge can be taken back like any other.
    merge::undo(&db, ops[2], false).await.unwrap();
    assert_eq!(
        title(&db, "recording", 14).await,
        ("Hyperballad (2015 Remaster)".to_owned(), None)
    );
}
