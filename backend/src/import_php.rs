//! One-off import of a musicbanana-php database (MySQL/MariaDB, `mb_*` tables).
//!
//! Reading and writing are separate: [`read_mysql`] loads the legacy tables into
//! [`LegacyData`], [`write`] turns that into the new schema inside one
//! transaction. Mapping and data quirks are described in docs/design.md.

use std::collections::{HashMap, HashSet, hash_map::Entry};

use anyhow::{Context, bail, ensure};
use sqlx::{AssertSqlSafe, MySqlPool, PgPool, Postgres, Transaction};
use time::OffsetDateTime;

use crate::{
    auth,
    catalog::{name_key, repair_mojibake},
};

const CLIENT: &str = "import:php-2016";
const LISTEN_BATCH: usize = 5_000;

#[derive(Debug, Default)]
pub struct LegacyData {
    pub users: Vec<User>,
    pub artists: Vec<Artist>,
    pub albums: Vec<Album>,
    pub tracks: Vec<Track>,
    /// Rows of `mb_usertracks_<user_id>`.
    pub listens: Vec<Listen>,
}

#[derive(Debug)]
pub struct User {
    pub id: i64,
    pub name: String,
    pub md5_password: String,
    pub email: String,
    /// Unix seconds; 0 or missing means unknown.
    pub registered: Option<i64>,
    /// `;2;3` style list of user ids.
    pub friends: String,
}

#[derive(Debug)]
pub struct Artist {
    pub id: i64,
    /// 0 = standalone, otherwise the artist this one was merged into.
    pub link_to: i64,
    pub name: String,
    pub times_played: i64,
}

#[derive(Debug)]
pub struct Album {
    pub id: i64,
    pub link_to: i64,
    pub artist_id: i64,
    pub title: String,
    pub times_played: i64,
}

#[derive(Debug)]
pub struct Track {
    pub id: i64,
    pub link_to: i64,
    pub artist_id: i64,
    pub title: String,
    /// 0 = no album.
    pub album_id: i64,
    /// Seconds.
    pub length: i64,
    pub times_played: i64,
}

#[derive(Debug)]
pub struct Listen {
    pub user_id: i64,
    /// Unix seconds.
    pub timestamp: i64,
    pub track_id: i64,
}

#[derive(Debug, Default)]
pub struct Report {
    pub accounts: usize,
    pub follows: usize,
    pub artists: usize,
    pub releases: usize,
    pub recordings: usize,
    pub listens: usize,
    /// Listens whose track no longer exists in `mb_tracks`.
    pub skipped_listens: usize,
    /// Listens in a `mb_usertracks_N` table without a matching user.
    pub orphaned_listens: usize,
    /// Artist, album and track rows whose name needed the mojibake repair.
    pub repaired_names: usize,
}

impl std::fmt::Display for Report {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "accounts:        {}", self.accounts)?;
        writeln!(f, "follows:         {}", self.follows)?;
        writeln!(f, "artists:         {}", self.artists)?;
        writeln!(f, "releases:        {}", self.releases)?;
        writeln!(f, "recordings:      {}", self.recordings)?;
        writeln!(f, "listens:         {}", self.listens)?;
        writeln!(
            f,
            "skipped listens: {} (track missing)",
            self.skipped_listens
        )?;
        writeln!(
            f,
            "orphaned:        {} (user missing)",
            self.orphaned_listens
        )?;
        write!(f, "repaired names:  {}", self.repaired_names)
    }
}

pub async fn read_mysql(url: &str) -> anyhow::Result<LegacyData> {
    let my = MySqlPool::connect(url)
        .await
        .context("connecting to MySQL")?;

    // Unsigned and TIMESTAMP columns are cast so they decode as plain i64.
    let users = sqlx::query_as::<_, (i64, String, String, String, Option<i64>, String)>(
        "SELECT CAST(id AS SIGNED), user, md5_password, email,
                CAST(UNIX_TIMESTAMP(registration) AS SIGNED), friends
           FROM mb_user ORDER BY id",
    )
    .fetch_all(&my)
    .await
    .context("reading mb_user")?
    .into_iter()
    .map(
        |(id, name, md5_password, email, registered, friends)| User {
            id,
            name,
            md5_password,
            email,
            registered: registered.filter(|&t| t > 0),
            friends,
        },
    )
    .collect();

    let artists = sqlx::query_as::<_, (i64, i64, String, i64)>(
        "SELECT CAST(id AS SIGNED), CAST(link_to_artist_id AS SIGNED), name,
                CAST(times_played AS SIGNED)
           FROM mb_artists ORDER BY id",
    )
    .fetch_all(&my)
    .await
    .context("reading mb_artists")?
    .into_iter()
    .map(|(id, link_to, name, times_played)| Artist {
        id,
        link_to,
        name,
        times_played,
    })
    .collect();

    let albums = sqlx::query_as::<_, (i64, i64, i64, String, i64)>(
        "SELECT CAST(id AS SIGNED), CAST(link_to_album_id AS SIGNED),
                CAST(artist_id AS SIGNED), title, CAST(times_played AS SIGNED)
           FROM mb_albums ORDER BY id",
    )
    .fetch_all(&my)
    .await
    .context("reading mb_albums")?
    .into_iter()
    .map(|(id, link_to, artist_id, title, times_played)| Album {
        id,
        link_to,
        artist_id,
        title,
        times_played,
    })
    .collect();

    let tracks = sqlx::query_as::<_, (i64, i64, i64, String, i64, i64, i64)>(
        "SELECT CAST(id AS SIGNED), CAST(link_to_track_id AS SIGNED),
                CAST(artist_id AS SIGNED), title, CAST(album_id AS SIGNED),
                CAST(length AS SIGNED), CAST(times_played AS SIGNED)
           FROM mb_tracks ORDER BY id",
    )
    .fetch_all(&my)
    .await
    .context("reading mb_tracks")?
    .into_iter()
    .map(
        |(id, link_to, artist_id, title, album_id, length, times_played)| Track {
            id,
            link_to,
            artist_id,
            title,
            album_id,
            length,
            times_played,
        },
    )
    .collect();

    let tables = sqlx::query_scalar::<_, String>(
        "SELECT CAST(table_name AS CHAR) FROM information_schema.tables
          WHERE table_schema = DATABASE() AND table_name LIKE 'mb\\_usertracks\\_%'",
    )
    .fetch_all(&my)
    .await
    .context("listing mb_usertracks_* tables")?;

    let mut listens = Vec::new();
    for table in tables {
        let Some(user_id) = table
            .strip_prefix("mb_usertracks_")
            .and_then(|n| n.parse::<i64>().ok())
        else {
            continue;
        };
        // Table name rebuilt from the parsed integer, so nothing from the database ends up in the SQL.
        let rows = sqlx::query_as::<_, (i64, i64)>(AssertSqlSafe(format!(
            "SELECT CAST(timestamp AS SIGNED), CAST(track_id AS SIGNED) FROM mb_usertracks_{user_id}"
        )))
        .fetch_all(&my)
        .await
        .with_context(|| format!("reading {table}"))?;
        listens.extend(rows.into_iter().map(|(timestamp, track_id)| Listen {
            user_id,
            timestamp,
            track_id,
        }));
    }

    Ok(LegacyData {
        users,
        artists,
        albums,
        tracks,
        listens,
    })
}

/// What a legacy track turns into on every listen.
struct TrackTarget {
    artist_id: i64,
    recording_id: i64,
    release_id: Option<i64>,
    artist_raw: String,
    track_raw: String,
    album_raw: Option<String>,
    duration_ms: Option<i32>,
}

pub async fn write(db: &PgPool, data: &LegacyData) -> anyhow::Result<Report> {
    let mut tx = db.begin().await?;
    let mut report = Report::default();

    let existing = sqlx::query_scalar!(
        r#"SELECT (SELECT count(*) FROM account) + (SELECT count(*) FROM listen) AS "n!""#
    )
    .fetch_one(&mut *tx)
    .await?;
    ensure!(
        existing == 0,
        "the target database already has accounts or listens; the import only runs into a fresh database"
    );

    let mut repaired = Repairs::default();

    // --- accounts, one default profile each
    let mut account_of_user = HashMap::new();
    let mut profile_of_user = HashMap::new();
    for user in &data.users {
        let email = match user.email.trim() {
            "" => format!("{}@legacy.invalid", user.name),
            e => e.to_lowercase(),
        };
        let password_hash = auth::wrap_legacy_md5(&user.md5_password)?;
        let registered = user
            .registered
            .map(OffsetDateTime::from_unix_timestamp)
            .transpose()?;
        let account_id = sqlx::query_scalar!(
            "INSERT INTO account (username, email, password_hash, password_legacy_md5, created_at)
             VALUES ($1::text, $2::text, $3, true, COALESCE($4, now()))
             RETURNING id",
            user.name,
            email,
            password_hash,
            registered,
        )
        .fetch_one(&mut *tx)
        .await
        .with_context(|| format!("importing user {} ({})", user.id, user.name))?;
        let profile_id = sqlx::query_scalar!(
            "INSERT INTO profile (account_id, slug, name, created_at)
             VALUES ($1, 'default', 'Default', COALESCE($2, now()))
             RETURNING id",
            account_id,
            registered,
        )
        .fetch_one(&mut *tx)
        .await?;
        account_of_user.insert(user.id, account_id);
        profile_of_user.insert(user.id, profile_id);
        report.accounts += 1;
    }

    for user in &data.users {
        for friend in user
            .friends
            .split(';')
            .filter_map(|f| f.trim().parse().ok())
        {
            let Some(&profile_id) = profile_of_user.get(&friend) else {
                continue;
            };
            let added = sqlx::query!(
                "INSERT INTO follow (follower_id, profile_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
                account_of_user[&user.id],
                profile_id,
            )
            .execute(&mut *tx)
            .await?;
            report.follows += added.rows_affected() as usize;
        }
    }

    // --- artists
    let artist_links: HashMap<i64, i64> = data.artists.iter().map(|a| (a.id, a.link_to)).collect();
    let artist_name: HashMap<i64, String> = data
        .artists
        .iter()
        .map(|a| (a.id, repaired.fix(&a.name)))
        .collect();
    let mut artist_by_key: HashMap<String, i64> = HashMap::new();
    let mut new_artist: HashMap<i64, i64> = HashMap::new();

    // Standalone entries first, most played first, so their spelling becomes the display name.
    let mut artists: Vec<&Artist> = data.artists.iter().collect();
    artists.sort_by_key(|a| (canonical(a.id, &artist_links) != a.id, -a.times_played));
    for a in artists {
        let target = canonical(a.id, &artist_links);
        let name = &artist_name[&a.id];
        let key = name_key(name);
        let id = if target != a.id {
            // Merged in the old system: its name becomes an alias of the merge target.
            new_artist[&target]
        } else if let Some(&id) = artist_by_key.get(&key) {
            id
        } else {
            let id =
                sqlx::query_scalar!("INSERT INTO artist (name) VALUES ($1) RETURNING id", name)
                    .fetch_one(&mut *tx)
                    .await?;
            report.artists += 1;
            id
        };
        if let Entry::Vacant(e) = artist_by_key.entry(key) {
            sqlx::query!(
                "INSERT INTO artist_alias (name_key, artist_id) VALUES ($1, $2)",
                e.key(),
                id
            )
            .execute(&mut *tx)
            .await?;
            e.insert(id);
        }
        new_artist.insert(a.id, id);
    }

    // --- albums → releases
    let album_links: HashMap<i64, i64> = data.albums.iter().map(|a| (a.id, a.link_to)).collect();
    let album_title: HashMap<i64, String> = data
        .albums
        .iter()
        .map(|a| (a.id, repaired.fix(&a.title)))
        .collect();
    let mut release_by_key: HashMap<(i64, String), i64> = HashMap::new();
    let mut new_release: HashMap<i64, i64> = HashMap::new();

    let mut albums: Vec<&Album> = data.albums.iter().collect();
    albums.sort_by_key(|a| (canonical(a.id, &album_links) != a.id, -a.times_played));
    for a in albums {
        let Some(&artist_id) = new_artist.get(&a.artist_id) else {
            continue;
        };
        let target = canonical(a.id, &album_links);
        let title = &album_title[&a.id];
        let key = (artist_id, name_key(title));
        let id = if let (true, Some(&id)) = (target != a.id, new_release.get(&target)) {
            id
        } else if let Some(&id) = release_by_key.get(&key) {
            id
        } else {
            let id = sqlx::query_scalar!(
                "INSERT INTO release (title, artist_id) VALUES ($1, $2) RETURNING id",
                title,
                artist_id
            )
            .fetch_one(&mut *tx)
            .await?;
            report.releases += 1;
            id
        };
        if let Entry::Vacant(e) = release_by_key.entry(key) {
            let (artist_id, title_key) = e.key();
            sqlx::query!(
                "INSERT INTO release_alias (artist_id, title_key, release_id) VALUES ($1, $2, $3)",
                artist_id,
                title_key,
                id
            )
            .execute(&mut *tx)
            .await?;
            e.insert(id);
        }
        new_release.insert(a.id, id);
    }

    // --- tracks → recordings (one per artist + title, independent of the album)
    let track_links: HashMap<i64, i64> = data.tracks.iter().map(|t| (t.id, t.link_to)).collect();
    let track_by_id: HashMap<i64, &Track> = data.tracks.iter().map(|t| (t.id, t)).collect();
    let mut recording_by_key: HashMap<(i64, String), i64> = HashMap::new();
    let mut new_recording: HashMap<i64, i64> = HashMap::new();
    let mut targets: HashMap<i64, TrackTarget> = HashMap::new();

    let mut tracks: Vec<&Track> = data.tracks.iter().collect();
    tracks.sort_by_key(|t| (canonical(t.id, &track_links) != t.id, -t.times_played));
    for t in tracks {
        // A merged track takes over its target's artist and album.
        let target = track_by_id[&canonical(t.id, &track_links)];
        let Some(&artist_id) = new_artist.get(&target.artist_id) else {
            continue;
        };
        let title = repaired.fix(&t.title);
        let key = (artist_id, name_key(&title));
        let length_ms = (t.length > 0).then(|| (t.length * 1000) as i32);
        let recording_id = if let (true, Some(&id)) =
            (target.id != t.id, new_recording.get(&target.id))
        {
            id
        } else if let Some(&id) = recording_by_key.get(&key) {
            id
        } else {
            let id = sqlx::query_scalar!(
                "INSERT INTO recording (title, artist_id, length_ms) VALUES ($1, $2, $3) RETURNING id",
                title,
                artist_id,
                length_ms
            )
            .fetch_one(&mut *tx)
            .await?;
            report.recordings += 1;
            id
        };
        if let Entry::Vacant(e) = recording_by_key.entry(key) {
            let (artist_id, title_key) = e.key();
            sqlx::query!(
                "INSERT INTO recording_alias (artist_id, title_key, recording_id) VALUES ($1, $2, $3)",
                artist_id,
                title_key,
                recording_id
            )
            .execute(&mut *tx)
            .await?;
            e.insert(recording_id);
        }
        new_recording.insert(t.id, recording_id);

        let album_id = if t.album_id != 0 {
            t.album_id
        } else {
            target.album_id
        };
        targets.insert(
            t.id,
            TrackTarget {
                artist_id,
                recording_id,
                release_id: new_release.get(&album_id).copied(),
                artist_raw: artist_name
                    .get(&t.artist_id)
                    .cloned()
                    .unwrap_or_else(|| artist_name[&target.artist_id].clone()),
                track_raw: title,
                album_raw: album_title.get(&t.album_id).cloned(),
                duration_ms: length_ms,
            },
        );
    }

    // --- listens
    let mut batch = ListenBatch::default();
    for listen in &data.listens {
        let Some(&profile_id) = profile_of_user.get(&listen.user_id) else {
            report.orphaned_listens += 1;
            continue;
        };
        let Some(target) = targets.get(&listen.track_id) else {
            report.skipped_listens += 1;
            continue;
        };
        batch.push(
            profile_id,
            OffsetDateTime::from_unix_timestamp(listen.timestamp)?,
            target,
        );
        if batch.len() >= LISTEN_BATCH {
            report.listens += batch.flush(&mut tx).await?;
        }
    }
    report.listens += batch.flush(&mut tx).await?;

    report.repaired_names = repaired.count;
    tx.commit().await?;
    Ok(report)
}

/// Follows `link_to` until a standalone entry (0, self-link or dangling link).
/// An entry whose chain runs into a cycle counts as standalone itself, so the
/// result is always an entry for which `canonical(x) == x`.
fn canonical(id: i64, links: &HashMap<i64, i64>) -> i64 {
    let mut seen = HashSet::from([id]);
    let mut current = id;
    while let Some(&next) = links.get(&current) {
        if next == 0 || next == current || !links.contains_key(&next) {
            return current;
        }
        if !seen.insert(next) {
            return id;
        }
        current = next;
    }
    current
}

#[derive(Default)]
struct Repairs {
    count: usize,
}

impl Repairs {
    fn fix(&mut self, s: &str) -> String {
        let fixed = repair_mojibake(s);
        if fixed != s {
            self.count += 1;
        }
        fixed.into_owned()
    }
}

#[derive(Default)]
struct ListenBatch {
    profile_id: Vec<i64>,
    listened_at: Vec<OffsetDateTime>,
    artist_raw: Vec<String>,
    track_raw: Vec<String>,
    album_raw: Vec<Option<String>>,
    duration_ms: Vec<Option<i32>>,
    artist_id: Vec<i64>,
    recording_id: Vec<i64>,
    release_id: Vec<Option<i64>>,
}

impl ListenBatch {
    fn len(&self) -> usize {
        self.profile_id.len()
    }

    fn push(&mut self, profile_id: i64, listened_at: OffsetDateTime, t: &TrackTarget) {
        self.profile_id.push(profile_id);
        self.listened_at.push(listened_at);
        self.artist_raw.push(t.artist_raw.clone());
        self.track_raw.push(t.track_raw.clone());
        self.album_raw.push(t.album_raw.clone());
        self.duration_ms.push(t.duration_ms);
        self.artist_id.push(t.artist_id);
        self.recording_id.push(t.recording_id);
        self.release_id.push(t.release_id);
    }

    async fn flush(&mut self, tx: &mut Transaction<'_, Postgres>) -> anyhow::Result<usize> {
        if self.len() == 0 {
            return Ok(0);
        }
        let batch = std::mem::take(self);
        let inserted = sqlx::query!(
            "INSERT INTO listen (profile_id, listened_at, artist_raw, track_raw, album_raw,
                                 duration_ms, client, artist_id, recording_id, release_id)
             SELECT t.profile_id, t.listened_at, t.artist_raw, t.track_raw, t.album_raw,
                    t.duration_ms, $10, t.artist_id, t.recording_id, t.release_id
               FROM UNNEST($1::int8[], $2::timestamptz[], $3::text[], $4::text[], $5::text[],
                           $6::int4[], $7::int8[], $8::int8[], $9::int8[])
                 AS t(profile_id, listened_at, artist_raw, track_raw, album_raw,
                      duration_ms, artist_id, recording_id, release_id)",
            &batch.profile_id,
            &batch.listened_at,
            &batch.artist_raw,
            &batch.track_raw,
            &batch.album_raw as &[Option<String>],
            &batch.duration_ms as &[Option<i32>],
            &batch.artist_id,
            &batch.recording_id,
            &batch.release_id as &[Option<i64>],
            CLIENT,
        )
        .execute(&mut **tx)
        .await?;
        let inserted = inserted.rows_affected() as usize;
        if inserted != batch.len() {
            bail!("expected {} listens, inserted {inserted}", batch.len());
        }
        Ok(inserted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_follows_chains_and_survives_cycles() {
        let links = HashMap::from([
            (1, 0),
            (2, 1),
            (3, 2),
            (4, 5),
            (5, 6),
            (6, 4),
            (7, 4),
            (8, 99),
            (9, 9),
        ]);
        assert_eq!(canonical(1, &links), 1);
        assert_eq!(canonical(3, &links), 1);
        assert_eq!(canonical(8, &links), 8, "dangling link stays put");
        assert_eq!(canonical(9, &links), 9, "self link");
        for id in [4, 5, 6, 7] {
            assert_eq!(
                canonical(id, &links),
                id,
                "cycles make every member standalone"
            );
        }
    }
}
