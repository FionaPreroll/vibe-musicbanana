//! Imports tests/fixtures/php2016.sql from a real MySQL/MariaDB server.
//!
//! Needs LEGACY_MYSQL_URL pointing at a server where the user may create
//! databases (e.g. mysql://root@127.0.0.1:3306). CI provides one; locally the
//! test is skipped when the variable is unset.

use musicbanana::{auth, import_php};
use sqlx::{Connection, MySqlConnection, PgPool};

async fn load_fixture() -> Option<String> {
    let Ok(server) = std::env::var("LEGACY_MYSQL_URL") else {
        eprintln!("LEGACY_MYSQL_URL not set, skipping");
        return None;
    };
    let server = server.trim_end_matches('/');
    let db = format!("mb_fixture_{}", std::process::id());

    let mut conn = MySqlConnection::connect(server).await.unwrap();
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "DROP DATABASE IF EXISTS {db}; CREATE DATABASE {db}"
    )))
    .execute(&mut conn)
    .await
    .unwrap();

    let url = format!("{server}/{db}");
    let mut conn = MySqlConnection::connect(&url).await.unwrap();
    sqlx::raw_sql(include_str!("fixtures/php2016.sql"))
        .execute(&mut conn)
        .await
        .unwrap();
    Some(url)
}

#[sqlx::test]
async fn imports_the_2016_php_database(db: PgPool) {
    let Some(url) = load_fixture().await else {
        return;
    };

    let data = import_php::read_mysql(&url).await.unwrap();
    let report = import_php::write(&db, &data).await.unwrap();

    assert_eq!(report.accounts, 3);
    assert_eq!(report.follows, 4, "friend 7 does not exist");
    // Mojibake and case duplicates collapse, old merges are followed.
    assert_eq!(report.artists, 3);
    assert_eq!(report.releases, 2);
    assert_eq!(report.recordings, 5);
    assert_eq!(report.listens, 10);
    assert_eq!(report.skipped_listens, 1, "track 99 does not exist");
    assert_eq!(report.orphaned_listens, 1, "mb_usertracks_9 has no user");
    assert_eq!(report.repaired_names, 2);

    let artists: Vec<String> = sqlx::query_scalar("SELECT name FROM artist ORDER BY name")
        .fetch_all(&db)
        .await
        .unwrap();
    assert_eq!(artists, ["Björk", "Die Ärzte", "Tiësto"]);

    // Every old spelling resolves through the alias table.
    for (spelling, artist) in [("dj tiësto", "Tiësto"), ("die ärzte", "Die Ärzte")] {
        let name: String = sqlx::query_scalar(
            "SELECT a.name FROM artist_alias x JOIN artist a ON a.id = x.artist_id WHERE x.name_key = $1",
        )
        .bind(spelling)
        .fetch_one(&db)
        .await
        .unwrap();
        assert_eq!(name, artist);
    }

    // Charts for fiona: all three spellings of Die Ärzte count together.
    let chart: Vec<(String, i64)> = sqlx::query_as(
        "SELECT a.name, count(*) FROM listen l
           JOIN profile p ON p.id = l.profile_id
           JOIN account u ON u.id = p.account_id
           JOIN artist a ON a.id = l.artist_id
          WHERE u.username = 'fiona'
          GROUP BY a.name ORDER BY count(*) DESC, a.name",
    )
    .fetch_all(&db)
    .await
    .unwrap();
    assert_eq!(
        chart,
        [
            ("Die Ärzte".to_owned(), 5),
            ("Björk".to_owned(), 2),
            ("Tiësto".to_owned(), 1),
        ]
    );

    // A listen of a double-encoded entry gets repaired raw strings and the shared release.
    let (artist_raw, album_raw, release): (String, Option<String>, Option<String>) =
        sqlx::query_as(
            "SELECT l.artist_raw, l.album_raw, rel.title FROM listen l
               LEFT JOIN release rel ON rel.id = l.release_id
              WHERE l.listened_at = to_timestamp(1187094800)",
        )
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(artist_raw, "Die Ärzte");
    assert_eq!(album_raw.as_deref(), Some("Geräusch"));
    assert_eq!(release.as_deref(), Some("Geräusch"));

    // Raw fields keep the (repaired) old spelling, the catalog link the merged entry.
    let (track_raw, recording, album_raw, release): (
        String,
        String,
        Option<String>,
        Option<String>,
    ) = sqlx::query_as(
        "SELECT l.track_raw, r.title, l.album_raw, rel.title
               FROM listen l JOIN recording r ON r.id = l.recording_id
               LEFT JOIN release rel ON rel.id = l.release_id
              WHERE l.listened_at = to_timestamp(1187096300)",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!((track_raw.as_str(), recording.as_str()), ("Joga", "Jóga"));
    assert_eq!((album_raw, release), (None, None));

    let (album_raw, release): (Option<String>, Option<String>) = sqlx::query_as(
        "SELECT l.album_raw, rel.title FROM listen l
           LEFT JOIN release rel ON rel.id = l.release_id
          WHERE l.listened_at = to_timestamp(1187096600)",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(album_raw.as_deref(), Some("Jazz ist anders (Bonus)"));
    assert_eq!(release.as_deref(), Some("Jazz ist anders"));

    // Old passwords keep working through the wrapped MD5.
    let (email, hash, legacy): (String, String, bool) = sqlx::query_as(
        "SELECT email::text, password_hash, password_legacy_md5 FROM account WHERE username = 'fiona'",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(email, "fiona@example.org");
    assert!(legacy);
    assert!(auth::verify_password(&hash, legacy, "banana"));

    let email: String =
        sqlx::query_scalar("SELECT email::text FROM account WHERE username = 'sam'")
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(email, "sam@legacy.invalid");

    // A second run must refuse instead of duplicating everything.
    assert!(import_php::write(&db, &data).await.is_err());
}
