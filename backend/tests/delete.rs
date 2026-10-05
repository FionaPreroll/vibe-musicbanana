//! Deleting profiles and accounts from the command line.

use musicbanana::delete;
use sqlx::PgPool;

// fixtures/profiles.sql: Fiona (account 1) with a public default profile and a
// private one, "arbeit"; alex (account 2) with a public default profile.

async fn count(db: &PgPool, sql: &'static str) -> i64 {
    sqlx::query_scalar(sql).fetch_one(db).await.unwrap()
}

#[sqlx::test(fixtures("profiles"))]
async fn deleting_a_profile(db: PgPool) {
    let arbeit = count(&db, "SELECT count(*) FROM listen WHERE profile_id = 2").await;
    let all = count(&db, "SELECT count(*) FROM listen").await;
    assert!(arbeit > 0);
    sqlx::query("INSERT INTO follow (follower_id, profile_id) VALUES (2, 2)")
        .execute(&db)
        .await
        .unwrap();

    // First only what would go.
    let would = delete::profile(&db, "fiona", "arbeit", false)
        .await
        .unwrap();
    assert_eq!(would.profiles, ["arbeit"]);
    assert_eq!(would.listens, arbeit);
    assert_eq!(would.followers, 1);
    assert_eq!(count(&db, "SELECT count(*) FROM listen").await, all);

    // The default profile goes with the account only.
    let refusal = delete::profile(&db, "Fiona", "default", true)
        .await
        .unwrap_err()
        .to_string();
    assert!(refusal.contains("account delete"), "{refusal}");
    assert!(delete::profile(&db, "fiona", "nope", true).await.is_err());

    delete::profile(&db, "Fiona", "arbeit", true).await.unwrap();
    assert_eq!(
        count(&db, "SELECT count(*) FROM profile WHERE id = 2").await,
        0
    );
    assert_eq!(count(&db, "SELECT count(*) FROM follow").await, 0);
    assert_eq!(
        count(&db, "SELECT count(*) FROM listen").await,
        all - arbeit
    );
}

#[sqlx::test(fixtures("profiles"))]
async fn deleting_an_account(db: PgPool) {
    let alex = count(&db, "SELECT count(*) FROM listen WHERE profile_id = 3").await;
    let all = count(&db, "SELECT count(*) FROM listen").await;
    sqlx::query("INSERT INTO follow (follower_id, profile_id) VALUES (2, 1)")
        .execute(&db)
        .await
        .unwrap();
    // A merge that changed one of alex's listens: undoing it later must not
    // bring the listen back.
    sqlx::query(
        "WITH op AS (INSERT INTO merge_op (kind, from_id, into_id, from_name, into_name)
                     VALUES ('artist', 3, 1, 'Tiësto', 'Die Ärzte') RETURNING id)
         INSERT INTO merge_change (op_id, tbl, action, key, old, new)
         SELECT op.id, 'listen', 'UPDATE', jsonb_build_object('id', l.id),
                '{\"artist_id\": 3}', '{\"artist_id\": 1}'
           FROM op, listen l WHERE l.profile_id = 3",
    )
    .execute(&db)
    .await
    .unwrap();

    let would = delete::account(&db, "alex", false).await.unwrap();
    assert_eq!(would.profiles, ["default"]);
    assert_eq!(would.listens, alex);
    assert_eq!(count(&db, "SELECT count(*) FROM account").await, 2);

    delete::account(&db, "ALEX", true).await.unwrap();
    assert_eq!(count(&db, "SELECT count(*) FROM account").await, 1);
    assert_eq!(count(&db, "SELECT count(*) FROM profile").await, 2);
    assert_eq!(count(&db, "SELECT count(*) FROM listen").await, all - alex);
    assert_eq!(count(&db, "SELECT count(*) FROM follow").await, 0);
    assert_eq!(count(&db, "SELECT count(*) FROM merge_change").await, 0);
    assert!(delete::account(&db, "alex", true).await.is_err());
}
