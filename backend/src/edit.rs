//! Changing catalog entries by hand: renaming them, and giving them MusicBrainz
//! IDs or taking those away.

use std::fmt;

use anyhow::{Context, bail, ensure};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use crate::{catalog::name_key, merge::Kind};

/// An entry as the messages name it: `artist 12 "Nirvana"`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Named {
    pub kind: Kind,
    pub id: i64,
    pub name: String,
}

impl fmt::Display for Named {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {} \"{}\"", self.kind, self.id, self.name)
    }
}

/// The entry `id` of `kind` with its artist (itself for an artist), locked
/// against merges until the end of the transaction.
async fn lock(conn: &mut PgConnection, kind: Kind, id: i64) -> anyhow::Result<(Named, i64)> {
    let row = match kind {
        Kind::Artist => sqlx::query!(
            "SELECT name, merged_into FROM artist WHERE id = $1 FOR NO KEY UPDATE",
            id
        )
        .fetch_optional(&mut *conn)
        .await?
        .map(|r| (r.name, id, r.merged_into)),
        Kind::Release => sqlx::query!(
            "SELECT title, artist_id, merged_into FROM release WHERE id = $1 FOR NO KEY UPDATE",
            id
        )
        .fetch_optional(&mut *conn)
        .await?
        .map(|r| (r.title, r.artist_id, r.merged_into)),
        Kind::Recording => sqlx::query!(
            "SELECT title, artist_id, merged_into FROM recording WHERE id = $1 FOR NO KEY UPDATE",
            id
        )
        .fetch_optional(&mut *conn)
        .await?
        .map(|r| (r.title, r.artist_id, r.merged_into)),
    };
    let (name, artist_id, merged_into) = row.with_context(|| format!("there is no {kind} {id}"))?;
    if let Some(other) = merged_into {
        bail!("{kind} {id} was merged into {other}; change that one instead");
    }
    Ok((Named { kind, id, name }, artist_id))
}

// ---------------------------------------------------------------- renaming

/// What [`rename`] changed.
#[derive(Debug)]
pub struct Renamed {
    /// The entry under its old name.
    pub entry: Named,
    pub name: String,
    /// The entry of the same kind (and artist) that the new name leads to,
    /// because it was a spelling of that one already.
    pub taken_by: Option<Named>,
}

impl fmt::Display for Renamed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Renamed {} to \"{}\".", self.entry, self.name)?;
        if let Some(other) = &self.taken_by {
            write!(
                f,
                "\nListens named \"{}\" without a MusicBrainz ID keep counting for {other}. \
                 If the two are the same: musicbanana merge {} {} {}",
                self.name, other.kind, self.entry.id, other.id
            )?;
        }
        Ok(())
    }
}

/// Renames the entry `id` of `kind`. Its old spellings keep leading to it, so
/// later listens under the old name count for it too, and the new name becomes
/// one of them unless it leads to another entry already.
pub async fn rename(db: &PgPool, kind: Kind, id: i64, name: &str) -> anyhow::Result<Renamed> {
    let name = name.trim();
    let key = name_key(name);
    ensure!(!key.is_empty(), "the new name is empty");
    let mut tx = db.begin().await?;
    let (entry, artist_id) = lock(&mut tx, kind, id).await?;
    let taken_by = match kind {
        Kind::Artist => {
            sqlx::query!("UPDATE artist SET name = $2 WHERE id = $1", id, name)
                .execute(&mut *tx)
                .await?;
            sqlx::query!(
                r#"WITH spelled AS (INSERT INTO artist_alias (name_key, artist_id) VALUES ($1, $2)
                                    ON CONFLICT DO NOTHING)
                   SELECT a.id, a.name FROM artist_alias x JOIN artist a ON a.id = x.artist_id
                    WHERE x.name_key = $1 AND a.id <> $2"#,
                key,
                id,
            )
            .fetch_optional(&mut *tx)
            .await?
            .map(|r| (r.id, r.name))
        }
        Kind::Release => {
            sqlx::query!("UPDATE release SET title = $2 WHERE id = $1", id, name)
                .execute(&mut *tx)
                .await?;
            sqlx::query!(
                r#"WITH spelled AS (INSERT INTO release_alias (artist_id, title_key, release_id)
                                    VALUES ($1, $2, $3)
                                    ON CONFLICT DO NOTHING)
                   SELECT r.id, r.title FROM release_alias x JOIN release r ON r.id = x.release_id
                    WHERE x.artist_id = $1 AND x.title_key = $2 AND r.id <> $3"#,
                artist_id,
                key,
                id,
            )
            .fetch_optional(&mut *tx)
            .await?
            .map(|r| (r.id, r.title))
        }
        Kind::Recording => {
            sqlx::query!("UPDATE recording SET title = $2 WHERE id = $1", id, name)
                .execute(&mut *tx)
                .await?;
            sqlx::query!(
                r#"WITH spelled AS (INSERT INTO recording_alias (artist_id, title_key, recording_id)
                                    VALUES ($1, $2, $3)
                                    ON CONFLICT DO NOTHING)
                   SELECT r.id, r.title FROM recording_alias x
                     JOIN recording r ON r.id = x.recording_id
                    WHERE x.artist_id = $1 AND x.title_key = $2 AND r.id <> $3"#,
                artist_id,
                key,
                id,
            )
            .fetch_optional(&mut *tx)
            .await?
            .map(|r| (r.id, r.title))
        }
    };
    tx.commit().await?;
    Ok(Renamed {
        entry,
        name: name.to_owned(),
        taken_by: taken_by.map(|(id, name)| Named { kind, id, name }),
    })
}

// ---------------------------------------------------------------- IDs

/// A MusicBrainz ID, given as such or as the address of its page, which must
/// then be of the right kind: an album (release) here is a release group there,
/// as a release in MusicBrainz is one edition of it.
pub fn parse_mbid(kind: Kind, text: &str) -> anyhow::Result<Uuid> {
    let address = text.trim().split(['?', '#']).next().unwrap_or_default();
    let segments: Vec<&str> = address.split('/').collect();
    let (at, mbid) = segments
        .iter()
        .enumerate()
        .find_map(|(at, s)| Uuid::try_parse(s).ok().map(|mbid| (at, mbid)))
        .with_context(|| format!("there is no MusicBrainz ID in \"{text}\""))?;
    let (page, what) = match kind {
        Kind::Artist => ("artist", "an artist"),
        Kind::Release => (
            "release-group",
            "a release group; an album takes the ID of the release group, which all \
             its editions share",
        ),
        Kind::Recording => ("recording", "a recording"),
    };
    if at > 0 {
        ensure!(
            segments[at - 1] == page,
            "\"{text}\" is not the page of {what} in MusicBrainz"
        );
    }
    Ok(mbid)
}

/// Gives the entry `id` of `kind` the MusicBrainz ID `mbid`, so that listens
/// with that ID count for it from then on. Returns the entry and whether it is
/// new to it.
pub async fn add_mbid(
    db: &PgPool,
    kind: Kind,
    id: i64,
    mbid: Uuid,
) -> anyhow::Result<(Named, bool)> {
    let mut tx = db.begin().await?;
    let (entry, artist_id) = lock(&mut tx, kind, id).await?;
    // The entry the ID belongs to so far. Albums and tracks are looked up by
    // artist and ID, as by artist and title.
    let owner = match kind {
        Kind::Artist => {
            sqlx::query_scalar!("SELECT artist_id FROM artist_mbid WHERE mbid = $1", mbid)
                .fetch_optional(&mut *tx)
                .await?
        }
        Kind::Release => {
            sqlx::query_scalar!(
                "SELECT release_id FROM release_mbid WHERE artist_id = $1 AND mbid = $2",
                artist_id,
                mbid
            )
            .fetch_optional(&mut *tx)
            .await?
        }
        Kind::Recording => {
            sqlx::query_scalar!(
                "SELECT recording_id FROM recording_mbid WHERE artist_id = $1 AND mbid = $2",
                artist_id,
                mbid
            )
            .fetch_optional(&mut *tx)
            .await?
        }
    };
    match owner {
        Some(owner) if owner == id => return Ok((entry, false)),
        Some(owner) => bail!(
            "the MusicBrainz ID {mbid} belongs to {kind} {owner}; take it from that one \
             first, or merge the two"
        ),
        None => {}
    }
    match kind {
        Kind::Artist => {
            sqlx::query!(
                "INSERT INTO artist_mbid (mbid, artist_id, name_key) VALUES ($1, $2, $3)",
                mbid,
                id,
                name_key(&entry.name),
            )
            .execute(&mut *tx)
            .await?;
        }
        Kind::Release => {
            sqlx::query!(
                "INSERT INTO release_mbid (artist_id, mbid, release_id) VALUES ($1, $2, $3)",
                artist_id,
                mbid,
                id,
            )
            .execute(&mut *tx)
            .await?;
        }
        Kind::Recording => {
            sqlx::query!(
                "INSERT INTO recording_mbid (artist_id, mbid, recording_id) VALUES ($1, $2, $3)",
                artist_id,
                mbid,
                id,
            )
            .execute(&mut *tx)
            .await?;
        }
    }
    tx.commit().await?;
    Ok((entry, true))
}

/// Takes the MusicBrainz ID `mbid` from the entry `id` of `kind`, such as one
/// that came to the wrong entry. The listens so far stay where they are; the
/// next one with the ID is matched as one with a new ID, see
/// [`crate::catalog::Resolver`].
pub async fn remove_mbid(db: &PgPool, kind: Kind, id: i64, mbid: Uuid) -> anyhow::Result<Named> {
    let mut tx = db.begin().await?;
    let (entry, _) = lock(&mut tx, kind, id).await?;
    let removed = match kind {
        Kind::Artist => sqlx::query!(
            "DELETE FROM artist_mbid WHERE mbid = $1 AND artist_id = $2",
            mbid,
            id
        )
        .execute(&mut *tx)
        .await?
        .rows_affected(),
        Kind::Release => sqlx::query!(
            "DELETE FROM release_mbid WHERE mbid = $1 AND release_id = $2",
            mbid,
            id
        )
        .execute(&mut *tx)
        .await?
        .rows_affected(),
        Kind::Recording => sqlx::query!(
            "DELETE FROM recording_mbid WHERE mbid = $1 AND recording_id = $2",
            mbid,
            id
        )
        .execute(&mut *tx)
        .await?
        .rows_affected(),
    };
    if removed == 0 {
        let has = mbids(&mut tx, kind, id).await?;
        if has.is_empty() {
            bail!("{entry} has no MusicBrainz ID");
        }
        let has: Vec<String> = has.iter().map(Uuid::to_string).collect();
        bail!(
            "{entry} does not have the MusicBrainz ID {mbid}, only {}",
            has.join(", ")
        );
    }
    tx.commit().await?;
    Ok(entry)
}

async fn mbids(conn: &mut PgConnection, kind: Kind, id: i64) -> sqlx::Result<Vec<Uuid>> {
    match kind {
        Kind::Artist => {
            sqlx::query_scalar!(
                "SELECT mbid FROM artist_mbid WHERE artist_id = $1 ORDER BY mbid",
                id
            )
            .fetch_all(&mut *conn)
            .await
        }
        Kind::Release => {
            sqlx::query_scalar!(
                "SELECT DISTINCT mbid FROM release_mbid WHERE release_id = $1 ORDER BY mbid",
                id
            )
            .fetch_all(&mut *conn)
            .await
        }
        Kind::Recording => {
            sqlx::query_scalar!(
                "SELECT DISTINCT mbid FROM recording_mbid WHERE recording_id = $1 ORDER BY mbid",
                id
            )
            .fetch_all(&mut *conn)
            .await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "5b11f4ce-a62d-471e-81fc-a69a8278c7da";

    #[test]
    fn takes_an_id_or_the_address_of_its_page() {
        let id = Uuid::try_parse(ID).unwrap();
        for text in [
            ID.to_owned(),
            format!(" {} ", ID.to_uppercase()),
            format!("https://musicbrainz.org/artist/{ID}"),
            format!("https://musicbrainz.org/artist/{ID}/"),
            format!("musicbrainz.org/artist/{ID}/recordings?page=2"),
        ] {
            assert_eq!(parse_mbid(Kind::Artist, &text).unwrap(), id, "{text}");
        }
        let album = format!("https://musicbrainz.org/release-group/{ID}");
        assert_eq!(parse_mbid(Kind::Release, &album).unwrap(), id);
        let track = format!("https://musicbrainz.org/recording/{ID}");
        assert_eq!(parse_mbid(Kind::Recording, &track).unwrap(), id);
    }

    #[test]
    fn refuses_other_pages_and_text() {
        let edition = format!("https://musicbrainz.org/release/{ID}");
        assert_eq!(
            parse_mbid(Kind::Release, &edition).unwrap_err().to_string(),
            format!(
                "\"{edition}\" is not the page of a release group; an album takes the ID of \
                 the release group, which all its editions share in MusicBrainz"
            )
        );
        let track = format!("https://musicbrainz.org/recording/{ID}");
        assert!(parse_mbid(Kind::Artist, &track).is_err());
        assert!(parse_mbid(Kind::Artist, "Nirvana").is_err());
        assert!(parse_mbid(Kind::Artist, "").is_err());
    }
}
