//! Taking merges back.

use musicbanana::{
    merge::{self, Kind, Options},
    scrobble::{self, Listen},
};
use serde_json::Value;
use sqlx::PgPool;
use time::OffsetDateTime;

// fixtures/merge.sql: "Die Aerzte" (artist 4) next to Die Ärzte (1), with
// Geräusch (release 3) and Unrockbar (recording 6) that Die Ärzte has as well;
// "Unrockbar (Live)" (recording 8) next to Unrockbar (1).

/// Every row of the catalog and the listens, to compare before and after.
async fn snapshot(db: &PgPool) -> Vec<Value> {
    let mut all = Vec::new();
    for sql in [
        "SELECT to_jsonb(t) FROM artist t",
        "SELECT to_jsonb(t) FROM release t",
        "SELECT to_jsonb(t) FROM recording t",
        "SELECT to_jsonb(t) FROM artist_alias t",
        "SELECT to_jsonb(t) FROM release_alias t",
        "SELECT to_jsonb(t) FROM recording_alias t",
        "SELECT to_jsonb(t) FROM artist_mbid t",
        "SELECT to_jsonb(t) FROM release_mbid t",
        "SELECT to_jsonb(t) FROM recording_mbid t",
        "SELECT to_jsonb(t) - 'submitted_at' FROM listen t",
    ] {
        let mut rows: Vec<Value> = sqlx::query_scalar(sql).fetch_all(db).await.unwrap();
        rows.sort_by_key(|r| r.to_string());
        all.push(Value::Array(rows));
    }
    all
}

async fn merge(db: &PgPool, kind: Kind, from: i64, into: i64) -> i64 {
    merge::merge(db, kind, from, into, Options::default())
        .await
        .unwrap()
        .op
        .unwrap()
}

/// A new listen in Fiona's default profile; returns its recording.
async fn scrobble(db: &PgPool, at: &str, artist: &str, track: &str) -> i64 {
    let listened_at =
        OffsetDateTime::parse(at, &time::format_description::well_known::Rfc3339).unwrap();
    let listen = Listen {
        listened_at,
        artist: artist.to_owned(),
        track: track.to_owned(),
        album: None,
        track_number: None,
        duration_ms: None,
        client: None,
        mbids: Default::default(),
        extra: None,
    };
    scrobble::record(db, 1, &[listen]).await.unwrap();
    sqlx::query_scalar("SELECT recording_id FROM listen WHERE listened_at = $1")
        .bind(listened_at)
        .fetch_one(db)
        .await
        .unwrap()
}

#[sqlx::test(fixtures("profiles", "merge"))]
async fn undoing_an_artist_merge_puts_everything_back(db: PgPool) {
    // Some MusicBrainz IDs, so that those move as well.
    sqlx::raw_sql(
        "INSERT INTO artist_mbid (mbid, artist_id, name_key)
         VALUES ('00000000-0000-0000-0000-000000000004', 4, 'die aerzte');
         INSERT INTO recording_mbid (artist_id, mbid, recording_id)
         VALUES (4, '00000000-0000-0000-0000-000000000007', 7)",
    )
    .execute(&db)
    .await
    .unwrap();
    let before = snapshot(&db).await;

    let op = merge(&db, Kind::Artist, 4, 1).await;
    assert_ne!(snapshot(&db).await, before);

    // A dry run tells and changes nothing.
    let dry = merge::undo(&db, op, true).await.unwrap();
    assert!(dry.restored > 0 && dry.kept == 0, "{dry}");
    assert_eq!(merge::log(&db, 10).await.unwrap()[0].undone_at, None);

    let undone = merge::undo(&db, op, false).await.unwrap();
    assert_eq!((undone.restored, undone.kept), (dry.restored, 0));
    assert_eq!(snapshot(&db).await, before);
    assert!(merge::log(&db, 10).await.unwrap()[0].undone_at.is_some());

    // Once is enough.
    let again = merge::undo(&db, op, false).await.unwrap_err();
    assert!(again.to_string().contains("undone already"), "{again}");
}

#[sqlx::test(fixtures("profiles", "merge"))]
async fn later_merges_of_the_same_entries_go_first(db: PgPool) {
    let before = snapshot(&db).await;
    let live = merge(&db, Kind::Recording, 8, 1).await;
    let middle = snapshot(&db).await;
    // Unrockbar, now with the live listens, into Deine Schuld: it takes those along.
    let both = merge(&db, Kind::Recording, 1, 2).await;

    let refused = merge::undo(&db, live, false).await.unwrap_err();
    assert_eq!(
        refused.to_string(),
        format!("merge {both} changed the same entries later; undo that one first")
    );

    merge::undo(&db, both, false).await.unwrap();
    assert_eq!(snapshot(&db).await, middle);
    merge::undo(&db, live, false).await.unwrap();
    assert_eq!(snapshot(&db).await, before);

    // The live version's spelling leads to it again.
    assert_eq!(
        scrobble(&db, "2024-01-01T12:00:00Z", "Die Ärzte", "Unrockbar (Live)").await,
        8
    );
}

#[sqlx::test(fixtures("profiles", "merge"))]
async fn what_changed_since_stays(db: PgPool) {
    let op = merge(&db, Kind::Recording, 8, 1).await;
    // A listen of the merged spelling after the merge counts for the remaining
    // track, and stays there: nothing tells that it meant the merged one.
    assert_eq!(
        scrobble(&db, "2024-01-01T12:00:00Z", "Die Ärzte", "Unrockbar (Live)").await,
        1
    );
    // The spelling is gone by hand since.
    sqlx::query("DELETE FROM recording_alias WHERE title_key = 'unrockbar (live)'")
        .execute(&db)
        .await
        .unwrap();

    let undone = merge::undo(&db, op, false).await.unwrap();
    assert_eq!(undone.kept, 1, "{undone}");
    let listens: Vec<(String, i64)> = sqlx::query_as(
        "SELECT listened_at::text, recording_id FROM listen
          WHERE track_raw = 'Unrockbar (Live)' ORDER BY listened_at",
    )
    .fetch_all(&db)
    .await
    .unwrap();
    assert_eq!(
        listens,
        [
            ("2016-05-03 12:00:00+00".to_owned(), 8),
            ("2024-01-01 12:00:00+00".to_owned(), 1)
        ]
    );
    let merged_into: Option<i64> =
        sqlx::query_scalar("SELECT merged_into FROM recording WHERE id = 8")
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(merged_into, None);
}

#[sqlx::test(fixtures("profiles", "merge"))]
async fn the_log_lists_merges_newest_first(db: PgPool) {
    merge(&db, Kind::Artist, 5, 2).await;
    merge::merge(
        &db,
        Kind::Recording,
        8,
        1,
        Options {
            dry_run: true,
            force: false,
        },
    )
    .await
    .unwrap();
    merge(&db, Kind::Recording, 8, 1).await;
    let log: Vec<String> = merge::log(&db, 10)
        .await
        .unwrap()
        .iter()
        .map(|op| format!("{} {} {} {}", op.kind, op.from.1, op.into.1, op.from.0))
        .collect();
    // The dry run left nothing behind.
    assert_eq!(
        log,
        [
            "recording Unrockbar (Live) Unrockbar 8",
            "artist Bjork Björk 5"
        ]
    );
    let fails = merge::undo(&db, 999, false).await.unwrap_err();
    assert!(
        fails.to_string().contains("there is no merge 999"),
        "{fails}"
    );
}
