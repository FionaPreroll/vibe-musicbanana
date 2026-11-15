//! Imports tests/fixtures/symfony2022.sql, a made-up musicbanana-symfony
//! database, into a musicbanana that already has listens from the PHP import
//! and from Navidrome. The old database goes into its own database on the
//! server of DATABASE_URL.

use musicbanana::{
    catalog::Mbids,
    import_symfony::{self, Mapping, Options, Target},
    scrobble::{self, Listen},
};
use sqlx::{Connection, PgConnection, PgPool};
use time::{OffsetDateTime, macros::datetime};

async fn load_fixture() -> String {
    let server = std::env::var("DATABASE_URL").expect("DATABASE_URL");
    let (base, _) = server.rsplit_once('/').unwrap();
    // One database per test, as the tests run side by side.
    let name = format!(
        "symfony_fixture_{}_{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    );
    let mut conn = PgConnection::connect(&server).await.unwrap();
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!("CREATE DATABASE {name}")))
        .execute(&mut conn)
        .await
        .unwrap();
    let url = format!("{base}/{name}");
    let mut conn = PgConnection::connect(&url).await.unwrap();
    sqlx::raw_sql(include_str!("fixtures/symfony2022.sql"))
        .execute(&mut conn)
        .await
        .unwrap();
    url
}

async fn account(db: &PgPool, username: &str) -> i64 {
    let account: i64 = sqlx::query_scalar(
        "INSERT INTO account (username, email, password_hash)
         VALUES ($1, $1 || '@example.org', 'x') RETURNING id",
    )
    .bind(username)
    .fetch_one(db)
    .await
    .unwrap();
    sqlx::query_scalar(
        "INSERT INTO profile (account_id, slug, name) VALUES ($1, 'default', 'Default') RETURNING id",
    )
    .bind(account)
    .fetch_one(db)
    .await
    .unwrap()
}

async fn listen(db: &PgPool, profile: i64, at: i64, artist: &str, track: &str, client: &str) {
    let listen = Listen {
        listened_at: OffsetDateTime::from_unix_timestamp(at).unwrap(),
        artist: artist.into(),
        track: track.into(),
        album: None,
        track_number: None,
        duration_ms: None,
        client: Some(client.into()),
        mbids: Mbids::default(),
        extra: None,
    };
    assert_eq!(scrobble::record(db, profile, &[listen]).await.unwrap(), 1);
}

/// fiona and emma as the PHP import and Navidrome left them.
async fn existing(db: &PgPool) -> (i64, i64) {
    let fiona = account(db, "fiona").await;
    let emma = account(db, "emma").await;
    listen(
        db,
        fiona,
        1_400_000_000,
        "Die Ärzte",
        "Schrei nach Liebe",
        "import:php-2016",
    )
    .await;
    listen(
        db,
        fiona,
        1_650_000_030,
        "Die Ärzte",
        "Westerland",
        "Navidrome 0.58.0",
    )
    .await;
    listen(
        db,
        emma,
        1_300_003_600,
        "Björk",
        "Hyperballad",
        "import:php-2016",
    )
    .await;
    (fiona, emma)
}

async fn count(db: &PgPool, sql: &str) -> i64 {
    sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
        .fetch_one(db)
        .await
        .unwrap()
}

#[sqlx::test]
async fn dry_run_matches_profiles_and_writes_nothing(db: PgPool) {
    existing(&db).await;
    let source = import_symfony::read(&load_fixture().await).await.unwrap();
    assert_eq!(source.profiles.len(), 5);
    assert_eq!(source.listens.len(), 9);

    let options = Options {
        dry_run: true,
        ..Options::default()
    };
    let report = import_symfony::import(&db, &source, &options)
        .await
        .unwrap();

    let targets: Vec<(&str, String)> = report
        .profiles
        .iter()
        .map(|p| (p.old.as_str(), p.target.to_string()))
        .collect();
    assert_eq!(
        targets,
        [
            ("fiona/_", "fiona/default".to_owned()),
            ("fiona/Arbeit", "fiona/arbeit (new profile)".to_owned()),
            ("fiona/leer", "fiona/leer (new profile)".to_owned()),
            ("Emma/emma", "emma/default".to_owned()),
            ("ghost/_", "nowhere, no account of that name".to_owned()),
        ]
    );
    let fiona = &report.profiles[0];
    assert_eq!(
        (
            fiona.listens,
            fiona.same_second,
            fiona.same_track,
            fiona.new,
            fiona.hours_off
        ),
        (4, 1, 1, 2, 0)
    );
    let emma = &report.profiles[3];
    assert_eq!(
        (
            emma.listens,
            emma.same_second,
            emma.same_track,
            emma.new,
            emma.hours_off
        ),
        (2, 0, 0, 2, 1)
    );
    assert_eq!(report.imported(), 0);

    assert_eq!(count(&db, "SELECT count(*) FROM listen").await, 3);
    assert_eq!(count(&db, "SELECT count(*) FROM profile").await, 2);
    assert!(report.to_string().ends_with("dry run, nothing written"));
}

#[sqlx::test]
async fn adds_what_is_new_and_only_once(db: PgPool) {
    let (fiona, emma) = existing(&db).await;
    let source = import_symfony::read(&load_fixture().await).await.unwrap();

    let report = import_symfony::import(&db, &source, &Options::default())
        .await
        .unwrap();
    let imported: Vec<u64> = report.profiles.iter().map(|p| p.imported).collect();
    // fiona: the same second and Navidrome's copy are left out.
    assert_eq!(imported, [2, 2, 0, 2, 0]);
    assert_eq!(report.repaired_names, 2);
    assert_eq!(report.profiles[4].target, Target::NoAccount);

    let fiona_listens: Vec<(i64, String, String, Option<i32>, String)> = sqlx::query_as(
        "SELECT extract(epoch FROM listened_at)::int8, artist_raw, track_raw, duration_ms, client
           FROM listen WHERE profile_id = $1 ORDER BY listened_at",
    )
    .bind(fiona)
    .fetch_all(&db)
    .await
    .unwrap();
    assert_eq!(
        fiona_listens,
        [
            (
                1_400_000_000,
                "Die Ärzte".into(),
                "Schrei nach Liebe".into(),
                None,
                "import:php-2016".into()
            ),
            (
                1_400_000_300,
                "Die Ärzte".into(),
                "Zu spät".into(),
                Some(150_000),
                "import:symfony-2022".into()
            ),
            (
                1_600_000_000,
                "Björk".into(),
                "Hyperballad".into(),
                Some(321_000),
                "import:symfony-2022".into()
            ),
            (
                1_650_000_030,
                "Die Ärzte".into(),
                "Westerland".into(),
                None,
                "Navidrome 0.58.0".into()
            ),
        ]
    );
    // The repaired spelling found the artist of the PHP import.
    assert_eq!(count(&db, "SELECT count(*) FROM artist").await, 2);

    let (slug, name, visibility): (String, String, String) = sqlx::query_as(
        "SELECT slug::text, name, visibility::text FROM profile
          WHERE account_id = (SELECT account_id FROM profile WHERE id = $1) AND slug <> 'default'",
    )
    .bind(fiona)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(
        (slug.as_str(), name.as_str(), visibility.as_str()),
        ("arbeit", "Arbeit", "private")
    );

    let emma_count: i64 = sqlx::query_scalar("SELECT count(*) FROM listen WHERE profile_id = $1")
        .bind(emma)
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(emma_count, 3);

    // A second run finds everything there already, the new profile included.
    let again = import_symfony::import(&db, &source, &Options::default())
        .await
        .unwrap();
    assert_eq!(again.imported(), 0);
    assert_eq!(again.profiles[1].target.to_string(), "fiona/arbeit");
    assert_eq!(count(&db, "SELECT count(*) FROM profile").await, 3);
}

#[sqlx::test]
async fn takes_a_time_range_and_explicit_profiles(db: PgPool) {
    let (_, emma) = existing(&db).await;
    let source = import_symfony::read(&load_fixture().await).await.unwrap();

    let options = Options {
        since: Some(datetime!(2016-05-31 0:00 UTC)),
        until: Some(datetime!(2021-01-01 0:00 UTC)),
        mappings: vec!["ghost/_=emma/default".parse().unwrap()],
        skip: vec!["fiona/Arbeit".parse().unwrap()],
        ..Options::default()
    };
    let report = import_symfony::import(&db, &source, &options)
        .await
        .unwrap();
    let listens: Vec<(usize, u64)> = report
        .profiles
        .iter()
        .map(|p| (p.listens, p.imported))
        .collect();
    // fiona: only 2020's Hyperballad; Arbeit is all 2021; leer is empty; emma: only 2017; ghost: 2021.
    assert_eq!(listens, [(1, 1), (0, 0), (0, 0), (1, 1), (0, 0)]);
    assert_eq!(report.profiles[1].target, Target::Skipped);
    assert_eq!(
        report.profiles[4].target,
        Target::Profile(emma, "emma/default".into())
    );
    // Nothing to put in, so no new profile for leer.
    assert_eq!(count(&db, "SELECT count(*) FROM profile").await, 2);

    let unknown = Options {
        mappings: vec![Mapping {
            old_user: "fiona".into(),
            old_profile: "Zuhause".into(),
            user: "fiona".into(),
            slug: "default".into(),
        }],
        ..Options::default()
    };
    let err = import_symfony::import(&db, &source, &unknown)
        .await
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "the old database has no profile fiona/Zuhause"
    );

    let missing = Options {
        mappings: vec!["ghost/_=emma/work".parse().unwrap()],
        ..Options::default()
    };
    let err = import_symfony::import(&db, &source, &missing)
        .await
        .unwrap_err();
    assert_eq!(err.to_string(), "user emma has no profile work");

    let skip_unknown = Options {
        skip: vec!["fiona/Zuhause".parse().unwrap()],
        ..Options::default()
    };
    let err = import_symfony::import(&db, &source, &skip_unknown)
        .await
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "the old database has no profile fiona/Zuhause"
    );
}

#[sqlx::test]
async fn follows_renamed_accounts(db: PgPool) {
    let (_, emma) = existing(&db).await;
    let source = import_symfony::read(&load_fixture().await).await.unwrap();

    let options = Options {
        users: vec!["ghost=emma".parse().unwrap()],
        ..Options::default()
    };
    let report = import_symfony::import(&db, &source, &options)
        .await
        .unwrap();
    let ghost = &report.profiles[4];
    assert_eq!(ghost.old, "ghost/_");
    assert_eq!(ghost.target, Target::Profile(emma, "emma/default".into()));
    assert_eq!(ghost.imported, 1);

    for (user, error) in [
        ("nobody=emma", "the old database has no profiles of nobody"),
        ("ghost=zoe", "there is no account zoe"),
    ] {
        let options = Options {
            users: vec![user.parse().unwrap()],
            ..Options::default()
        };
        let err = import_symfony::import(&db, &source, &options)
            .await
            .unwrap_err();
        assert_eq!(err.to_string(), error);
    }
}

#[sqlx::test]
async fn leaves_out_plays_another_profile_has(db: PgPool) {
    let (fiona, _) = existing(&db).await;
    let work: i64 = sqlx::query_scalar(
        "INSERT INTO profile (account_id, slug, name)
         SELECT account_id, 'work', 'Work' FROM profile WHERE id = $1 RETURNING id",
    )
    .bind(fiona)
    .fetch_one(&db)
    .await
    .unwrap();
    // The two plays of the old profile Arbeit, scrobbled to another profile:
    // one at the same second, one a few seconds later.
    listen(
        &db,
        work,
        1_620_000_000,
        "Björk",
        "Hyperballad",
        "Navidrome 0.58.0",
    )
    .await;
    listen(
        &db,
        work,
        1_620_000_410,
        "björk",
        "hyperballad",
        "Navidrome 0.58.0",
    )
    .await;
    let source = import_symfony::read(&load_fixture().await).await.unwrap();

    let dry = Options {
        dry_run: true,
        ..Options::default()
    };
    let report = import_symfony::import(&db, &source, &dry).await.unwrap();
    let arbeit = &report.profiles[1];
    assert_eq!(
        (
            arbeit.listens,
            arbeit.same_second,
            arbeit.same_track,
            arbeit.new
        ),
        (2, 1, 1, 0)
    );

    let report = import_symfony::import(&db, &source, &Options::default())
        .await
        .unwrap();
    assert_eq!(report.profiles[1].imported, 0);
    assert_eq!(report.imported(), 4);
    // Nothing new for Arbeit, so no profile for it.
    assert_eq!(
        count(&db, "SELECT count(*) FROM profile WHERE slug = 'arbeit'").await,
        0
    );
}
