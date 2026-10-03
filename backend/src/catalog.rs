//! Matching scrobbled strings to catalog entries.

use std::{borrow::Cow, collections::HashMap};

use sqlx::PgPool;
use unicode_normalization::UnicodeNormalization;

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

/// Finds the catalog entries for scrobbled names through the alias tables and
/// creates the missing ones (entry plus alias). Remembers what it resolved, so a
/// batch of listens asks the database once per name.
///
/// Each lookup is one statement that either finds the alias or inserts the entry
/// together with its alias. When two requests create the same name at the same
/// time, the slower one fails on the alias key, which also undoes its entry, and
/// the retry finds the alias of the faster one.
pub struct Resolver<'a> {
    db: &'a PgPool,
    artists: HashMap<String, i64>,
    releases: HashMap<(i64, String), i64>,
    recordings: HashMap<(i64, String), i64>,
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

    pub async fn artist(&mut self, name: &str) -> sqlx::Result<i64> {
        let key = name_key(name);
        if let Some(&id) = self.artists.get(&key) {
            return Ok(id);
        }
        let name = name.trim();
        let id = retry_on_conflict(|| {
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
                &key,
                name,
            )
            .fetch_one(self.db)
        })
        .await?;
        self.artists.insert(key, id);
        Ok(id)
    }

    /// The album `title` of the artist. Clients only send the track's artist, so
    /// that is the album artist here as well.
    pub async fn release(&mut self, artist_id: i64, title: &str) -> sqlx::Result<i64> {
        let key = (artist_id, name_key(title));
        if let Some(&id) = self.releases.get(&key) {
            return Ok(id);
        }
        let title = title.trim();
        let id = retry_on_conflict(|| {
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
                &key.1,
                title,
            )
            .fetch_one(self.db)
        })
        .await?;
        self.releases.insert(key, id);
        Ok(id)
    }

    /// The track `title` of the artist, whatever album it was played from.
    /// `length_ms` is only used when the recording is new.
    pub async fn recording(
        &mut self,
        artist_id: i64,
        title: &str,
        length_ms: Option<i32>,
    ) -> sqlx::Result<i64> {
        let key = (artist_id, name_key(title));
        if let Some(&id) = self.recordings.get(&key) {
            return Ok(id);
        }
        let title = title.trim();
        let id = retry_on_conflict(|| {
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
                &key.1,
                title,
                length_ms,
            )
            .fetch_one(self.db)
        })
        .await?;
        self.recordings.insert(key, id);
        Ok(id)
    }
}

/// Runs a find-or-create statement again after a unique violation, i.e. when a
/// concurrent request created the same alias first.
async fn retry_on_conflict<F, Fut>(mut statement: F) -> sqlx::Result<i64>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = sqlx::Result<i64>>,
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
