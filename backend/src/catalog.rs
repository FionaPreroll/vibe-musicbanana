//! Matching scrobbled strings to catalog entries.

use std::{borrow::Cow, collections::HashMap};

use sqlx::PgPool;
use unicode_normalization::UnicodeNormalization;
use uuid::Uuid;

/// Key under which a name is looked up in the `*_alias` tables: NFKC, lowercase,
/// trimmed, inner whitespace collapsed. Accents stay significant ("Jóga" ≠ "Joga").
pub fn name_key(name: &str) -> String {
    let normalized: String = name.nfkc().collect::<String>().to_lowercase();
    normalized.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Undoes UTF-8 that was read as Windows-1252 and stored as UTF-8 again
/// ("Die Ã„rzte" → "Die Ärzte", "SÃ¶hne Mannheims" → "Söhne Mannheims").
///
/// Only applied when the round trip yields valid UTF-8, so correct names such as
/// "Björk" (whose bytes are not valid UTF-8 once mapped back) stay untouched.
/// Repeats to undo multiple layers.
pub fn repair_mojibake(s: &str) -> Cow<'_, str> {
    let mut out = Cow::Borrowed(s);
    for _ in 0..3 {
        match undo_one_layer(&out) {
            Some(fixed) if fixed != *out => out = Cow::Owned(fixed),
            _ => break,
        }
    }
    out
}

fn undo_one_layer(s: &str) -> Option<String> {
    if s.is_ascii() {
        return None;
    }
    let bytes = s.chars().map(cp1252_byte).collect::<Option<Vec<u8>>>()?;
    String::from_utf8(bytes).ok()
}

/// The byte a character came from when bytes were decoded as Windows-1252
/// (MySQL's "latin1", which also passes 0x81, 0x8D, 0x8F, 0x90, 0x9D through).
fn cp1252_byte(c: char) -> Option<u8> {
    let b = match c {
        '\u{0}'..='\u{FF}' => c as u8,
        '€' => 0x80,
        '‚' => 0x82,
        'ƒ' => 0x83,
        '„' => 0x84,
        '…' => 0x85,
        '†' => 0x86,
        '‡' => 0x87,
        'ˆ' => 0x88,
        '‰' => 0x89,
        'Š' => 0x8A,
        '‹' => 0x8B,
        'Œ' => 0x8C,
        'Ž' => 0x8E,
        '‘' => 0x91,
        '’' => 0x92,
        '“' => 0x93,
        '”' => 0x94,
        '•' => 0x95,
        '–' => 0x96,
        '—' => 0x97,
        '˜' => 0x98,
        '™' => 0x99,
        'š' => 0x9A,
        '›' => 0x9B,
        'œ' => 0x9C,
        'ž' => 0x9E,
        'Ÿ' => 0x9F,
        _ => return None,
    };
    Some(b)
}

/// The MusicBrainz IDs a client sent along with a listen.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Mbids {
    /// The artist, when the track has only one.
    pub artist: Option<Uuid>,
    /// The album's release group, which all its editions share.
    pub release_group: Option<Uuid>,
    pub recording: Option<Uuid>,
}

/// Finds the catalog entries for scrobbled names through the alias tables and
/// creates the missing ones (entry plus alias). Remembers what it resolved, so a
/// batch of listens asks the database once per name.
///
/// A MusicBrainz ID that comes with a name tells entries of the same name apart:
/// an artist, album or track with another ID is another one. An unknown ID goes
/// to the entry the name leads to if that has none yet, so what was imported or
/// scrobbled without IDs stays with the first ID that turns up. The entry of a
/// second ID gets no alias, so listens without an ID keep going to the first one.
///
/// Each lookup is one statement that either finds the alias or inserts the entry
/// together with its alias. When two requests create the same name at the same
/// time, the slower one fails on the alias key, which also undoes its entry, and
/// the retry finds the alias of the faster one. Lookups by ID work alike with the
/// ID as the key, in a transaction.
pub struct Resolver<'a> {
    db: &'a PgPool,
    artists: HashMap<(String, Option<Uuid>), i64>,
    releases: HashMap<(i64, String, Option<Uuid>), i64>,
    recordings: HashMap<(i64, String, Option<Uuid>), i64>,
}

impl<'a> Resolver<'a> {
    pub fn new(db: &'a PgPool) -> Self {
        Self {
            db,
            artists: HashMap::new(),
            releases: HashMap::new(),
            recordings: HashMap::new(),
        }
    }

    /// The artist of this name, or the one of the MusicBrainz ID if the name fits it.
    pub async fn artist(&mut self, name: &str, mbid: Option<Uuid>) -> sqlx::Result<i64> {
        let key = (name_key(name), mbid);
        if let Some(&id) = self.artists.get(&key) {
            return Ok(id);
        }
        let name = name.trim();
        let by_mbid = match mbid {
            Some(mbid) => retry_on_conflict(|| artist_by_mbid(self.db, &key.0, name, mbid)).await?,
            None => None,
        };
        let id = match by_mbid {
            Some(id) => id,
            None => retry_on_conflict(|| artist_by_name(self.db, &key.0, name)).await?,
        };
        self.artists.insert(key, id);
        Ok(id)
    }

    /// The album `title` of the artist, or the one of the release group `mbid`.
    /// Clients only send the track's artist, so that is the album artist here as well.
    pub async fn release(
        &mut self,
        artist_id: i64,
        title: &str,
        mbid: Option<Uuid>,
    ) -> sqlx::Result<i64> {
        let key = (artist_id, name_key(title), mbid);
        if let Some(&id) = self.releases.get(&key) {
            return Ok(id);
        }
        let title = title.trim();
        let id = match mbid {
            Some(mbid) => {
                retry_on_conflict(|| release_by_mbid(self.db, artist_id, &key.1, title, mbid))
                    .await?
            }
            None => {
                retry_on_conflict(|| release_by_title(self.db, artist_id, &key.1, title)).await?
            }
        };
        self.releases.insert(key, id);
        Ok(id)
    }

    /// The track `title` of the artist, whatever album it was played from, or the
    /// one of the recording `mbid`. `length_ms` is only used when the track is new.
    pub async fn recording(
        &mut self,
        artist_id: i64,
        title: &str,
        length_ms: Option<i32>,
        mbid: Option<Uuid>,
    ) -> sqlx::Result<i64> {
        let key = (artist_id, name_key(title), mbid);
        if let Some(&id) = self.recordings.get(&key) {
            return Ok(id);
        }
        let title = title.trim();
        let id = match mbid {
            Some(mbid) => {
                retry_on_conflict(|| {
                    recording_by_mbid(self.db, artist_id, &key.1, title, length_ms, mbid)
                })
                .await?
            }
            None => {
                retry_on_conflict(|| {
                    recording_by_title(self.db, artist_id, &key.1, title, length_ms)
                })
                .await?
            }
        };
        self.recordings.insert(key, id);
        Ok(id)
    }
}

async fn artist_by_name(db: &PgPool, key: &str, name: &str) -> sqlx::Result<i64> {
    sqlx::query_scalar!(
        r#"WITH found AS (SELECT artist_id FROM artist_alias WHERE name_key = $1),
                created AS (INSERT INTO artist (name)
                            SELECT $2 WHERE NOT EXISTS (SELECT FROM found)
                            RETURNING id),
                aliased AS (INSERT INTO artist_alias (name_key, artist_id)
                            SELECT $1, id FROM created
                            RETURNING artist_id)
           SELECT artist_id AS "id!" FROM found
           UNION ALL
           SELECT artist_id FROM aliased"#,
        key,
        name,
    )
    .fetch_one(db)
    .await
}

/// The artist with the MusicBrainz ID `mbid`, or `None` when the ID is known for
/// other names of an artist only: its spellings, its name, and the name it had
/// when the ID came to it. Clients pair names and IDs of a track by several
/// artists badly; one may send the ID of A along with "A & B", and then neither
/// may the listens of A go to "A & B" nor the other way round.
///
/// An unknown ID goes to the artist of the name if that has no ID yet, else to a
/// new artist of the same name. The name's artist stays locked meanwhile, so that
/// two new IDs cannot both take it.
async fn artist_by_mbid(
    db: &PgPool,
    key: &str,
    name: &str,
    mbid: Uuid,
) -> sqlx::Result<Option<i64>> {
    let known = sqlx::query!(
        r#"SELECT a.id, a.name, m.name_key AS came_with,
                  EXISTS (SELECT FROM artist_alias x
                           WHERE x.name_key = $2 AND x.artist_id = a.id) AS "spelled!"
             FROM artist_mbid m
             JOIN artist a ON a.id = m.artist_id
            WHERE m.mbid = $1"#,
        mbid,
        key,
    )
    .fetch_optional(db)
    .await?;
    if let Some(known) = known {
        let named = known.spelled || known.came_with == key || name_key(&known.name) == key;
        return Ok(named.then_some(known.id));
    }

    let mut tx = db.begin().await?;
    let named = sqlx::query_scalar!(
        "SELECT a.id FROM artist_alias x JOIN artist a ON a.id = x.artist_id
          WHERE x.name_key = $1 AND a.merged_into IS NULL
            FOR NO KEY UPDATE OF a",
        key,
    )
    .fetch_optional(&mut *tx)
    .await?;
    let id = match named {
        Some(id) => {
            let tagged = sqlx::query_scalar!(
                r#"SELECT EXISTS (SELECT FROM artist_mbid WHERE artist_id = $1) AS "tagged!""#,
                id,
            )
            .fetch_one(&mut *tx)
            .await?;
            if tagged {
                // Another artist of this name; the name keeps leading to the first one.
                sqlx::query_scalar!("INSERT INTO artist (name) VALUES ($1) RETURNING id", name)
                    .fetch_one(&mut *tx)
                    .await?
            } else {
                id
            }
        }
        None => {
            sqlx::query_scalar!(
                r#"WITH created AS (INSERT INTO artist (name) VALUES ($2) RETURNING id)
                   INSERT INTO artist_alias (name_key, artist_id)
                   SELECT $1, id FROM created
                   RETURNING artist_id"#,
                key,
                name,
            )
            .fetch_one(&mut *tx)
            .await?
        }
    };
    sqlx::query!(
        "INSERT INTO artist_mbid (mbid, artist_id, name_key) VALUES ($1, $2, $3)",
        mbid,
        id,
        key,
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Some(id))
}

async fn release_by_title(
    db: &PgPool,
    artist_id: i64,
    key: &str,
    title: &str,
) -> sqlx::Result<i64> {
    sqlx::query_scalar!(
        r#"WITH found AS (SELECT release_id FROM release_alias
                           WHERE artist_id = $1 AND title_key = $2),
                created AS (INSERT INTO release (title, artist_id)
                            SELECT $3, $1 WHERE NOT EXISTS (SELECT FROM found)
                            RETURNING id),
                aliased AS (INSERT INTO release_alias (artist_id, title_key, release_id)
                            SELECT $1, $2, id FROM created
                            RETURNING release_id)
           SELECT release_id AS "id!" FROM found
           UNION ALL
           SELECT release_id FROM aliased"#,
        artist_id,
        key,
        title,
    )
    .fetch_one(db)
    .await
}

/// The artist's album of the release group `mbid`, under whatever title; a new
/// title becomes one of its spellings. An unknown ID goes to the album of the
/// title if that has no ID yet, else to a new album of the same title, such as
/// a single named like the album it is from.
async fn release_by_mbid(
    db: &PgPool,
    artist_id: i64,
    key: &str,
    title: &str,
    mbid: Uuid,
) -> sqlx::Result<i64> {
    let known = sqlx::query_scalar!(
        r#"WITH known AS (SELECT release_id FROM release_mbid
                           WHERE artist_id = $1 AND mbid = $2),
                spelled AS (INSERT INTO release_alias (artist_id, title_key, release_id)
                            SELECT $1, $3, release_id FROM known
                            ON CONFLICT DO NOTHING)
           SELECT release_id AS "id!" FROM known"#,
        artist_id,
        mbid,
        key,
    )
    .fetch_optional(db)
    .await?;
    if let Some(id) = known {
        return Ok(id);
    }

    let mut tx = db.begin().await?;
    let named = sqlx::query_scalar!(
        "SELECT r.id FROM release_alias x JOIN release r ON r.id = x.release_id
          WHERE x.artist_id = $1 AND x.title_key = $2 AND r.merged_into IS NULL
            FOR NO KEY UPDATE OF r",
        artist_id,
        key,
    )
    .fetch_optional(&mut *tx)
    .await?;
    let id = match named {
        Some(id) => {
            let tagged = sqlx::query_scalar!(
                r#"SELECT EXISTS (SELECT FROM release_mbid WHERE release_id = $1) AS "tagged!""#,
                id,
            )
            .fetch_one(&mut *tx)
            .await?;
            if tagged {
                sqlx::query_scalar!(
                    "INSERT INTO release (title, artist_id) VALUES ($1, $2) RETURNING id",
                    title,
                    artist_id,
                )
                .fetch_one(&mut *tx)
                .await?
            } else {
                id
            }
        }
        None => {
            sqlx::query_scalar!(
                r#"WITH created AS (INSERT INTO release (title, artist_id) VALUES ($3, $1)
                                    RETURNING id)
                   INSERT INTO release_alias (artist_id, title_key, release_id)
                   SELECT $1, $2, id FROM created
                   RETURNING release_id"#,
                artist_id,
                key,
                title,
            )
            .fetch_one(&mut *tx)
            .await?
        }
    };
    sqlx::query!(
        "INSERT INTO release_mbid (artist_id, mbid, release_id) VALUES ($1, $2, $3)",
        artist_id,
        mbid,
        id,
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(id)
}

async fn recording_by_title(
    db: &PgPool,
    artist_id: i64,
    key: &str,
    title: &str,
    length_ms: Option<i32>,
) -> sqlx::Result<i64> {
    sqlx::query_scalar!(
        r#"WITH found AS (SELECT recording_id FROM recording_alias
                           WHERE artist_id = $1 AND title_key = $2),
                created AS (INSERT INTO recording (title, artist_id, length_ms)
                            SELECT $3, $1, $4 WHERE NOT EXISTS (SELECT FROM found)
                            RETURNING id),
                aliased AS (INSERT INTO recording_alias (artist_id, title_key, recording_id)
                            SELECT $1, $2, id FROM created
                            RETURNING recording_id)
           SELECT recording_id AS "id!" FROM found
           UNION ALL
           SELECT recording_id FROM aliased"#,
        artist_id,
        key,
        title,
        length_ms,
    )
    .fetch_one(db)
    .await
}

/// The artist's track of the recording `mbid`, under whatever title; a new title
/// becomes one of its spellings. An unknown ID goes to the track of the title if
/// that has no ID yet, else to a new track of the same title. A recording in
/// MusicBrainz is the same audio on whatever album: a remaster mostly keeps the
/// ID of the original, while a live version or a new recording has its own.
async fn recording_by_mbid(
    db: &PgPool,
    artist_id: i64,
    key: &str,
    title: &str,
    length_ms: Option<i32>,
    mbid: Uuid,
) -> sqlx::Result<i64> {
    let known = sqlx::query_scalar!(
        r#"WITH known AS (SELECT recording_id FROM recording_mbid
                           WHERE artist_id = $1 AND mbid = $2),
                spelled AS (INSERT INTO recording_alias (artist_id, title_key, recording_id)
                            SELECT $1, $3, recording_id FROM known
                            ON CONFLICT DO NOTHING)
           SELECT recording_id AS "id!" FROM known"#,
        artist_id,
        mbid,
        key,
    )
    .fetch_optional(db)
    .await?;
    if let Some(id) = known {
        return Ok(id);
    }

    let mut tx = db.begin().await?;
    let named = sqlx::query_scalar!(
        "SELECT r.id FROM recording_alias x JOIN recording r ON r.id = x.recording_id
          WHERE x.artist_id = $1 AND x.title_key = $2 AND r.merged_into IS NULL
            FOR NO KEY UPDATE OF r",
        artist_id,
        key,
    )
    .fetch_optional(&mut *tx)
    .await?;
    let id = match named {
        Some(id) => {
            let tagged = sqlx::query_scalar!(
                r#"SELECT EXISTS (SELECT FROM recording_mbid WHERE recording_id = $1) AS "tagged!""#,
                id,
            )
            .fetch_one(&mut *tx)
            .await?;
            if tagged {
                sqlx::query_scalar!(
                    "INSERT INTO recording (title, artist_id, length_ms) VALUES ($1, $2, $3)
                     RETURNING id",
                    title,
                    artist_id,
                    length_ms,
                )
                .fetch_one(&mut *tx)
                .await?
            } else {
                id
            }
        }
        None => {
            sqlx::query_scalar!(
                r#"WITH created AS (INSERT INTO recording (title, artist_id, length_ms)
                                    VALUES ($3, $1, $4)
                                    RETURNING id)
                   INSERT INTO recording_alias (artist_id, title_key, recording_id)
                   SELECT $1, $2, id FROM created
                   RETURNING recording_id"#,
                artist_id,
                key,
                title,
                length_ms,
            )
            .fetch_one(&mut *tx)
            .await?
        }
    };
    sqlx::query!(
        "INSERT INTO recording_mbid (artist_id, mbid, recording_id) VALUES ($1, $2, $3)",
        artist_id,
        mbid,
        id,
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(id)
}

/// Runs a find-or-create again after a unique violation, i.e. when a concurrent
/// request created the same alias or took the same ID first.
async fn retry_on_conflict<T, F, Fut>(mut statement: F) -> sqlx::Result<T>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = sqlx::Result<T>>,
{
    let mut retries = 0;
    loop {
        match statement().await {
            Err(sqlx::Error::Database(err)) if err.is_unique_violation() && retries < 3 => {
                retries += 1;
            }
            result => return result,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repairs_double_encoded_names() {
        assert_eq!(repair_mojibake("SÃ¶hne Mannheims"), "Söhne Mannheims");
        assert_eq!(repair_mojibake("Kein KÃ¼nstler"), "Kein Künstler");
        assert_eq!(repair_mojibake("TiÃ«sto"), "Tiësto");
        // Ä is C3 84, and 0x84 is „ in Windows-1252.
        assert_eq!(repair_mojibake("Die Ã„rzte"), "Die Ärzte");
        // ß is C3 9F, and 0x9F is Ÿ in Windows-1252.
        assert_eq!(repair_mojibake("WeiÃŸ"), "Weiß");
        assert_eq!(repair_mojibake("SÃƒÂ¶hne"), "Söhne");
    }

    #[test]
    fn leaves_correct_names_alone() {
        for name in [
            "Söhne Mannheims",
            "Björk",
            "Die Ärzte",
            "Sigur Rós",
            "Motörhead",
            "AC/DC",
            "Café del Mar",
            "東京事変",
            "",
        ] {
            assert!(matches!(repair_mojibake(name), Cow::Borrowed(_)), "{name}");
        }
    }

    #[test]
    fn name_key_folds_case_and_whitespace_but_not_accents() {
        assert_eq!(name_key("  Die   ÄRZTE "), "die ärzte");
        assert_eq!(name_key("björk"), name_key("Björk"));
        assert_ne!(name_key("Jóga"), name_key("Joga"));
        // NFKC: composed and decomposed forms match.
        assert_eq!(name_key("Bjo\u{308}rk"), name_key("Björk"));
    }
}
