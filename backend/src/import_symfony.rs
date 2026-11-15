//! Import of the musicbanana-symfony database (2021-2022, PostgreSQL).
//!
//! Unlike the PHP import this one adds to a musicbanana that is in use: the old
//! profiles are matched to the accounts and profiles of the same names, and their
//! listens go in through [`scrobble::record`], like those of a scrobbler. So a
//! listen the profile has at the same second (the PHP import of the same years)
//! or a few moments apart (Navidrome, YourSpotify) is left out, and so is one the
//! owner moved to the trash. That also makes a second run harmless.
//!
//! [`read`] loads the old database, [`import`] matches and writes. With
//! `dry_run` it only matches and counts how the old listens line up with those
//! already there, including at a whole hour or two off, which would mean one of
//! the imports stored local time.

use std::{collections::HashMap, fmt, str::FromStr};

use anyhow::{Context, bail};
use sqlx::PgPool;
use time::{Date, OffsetDateTime, Time};

use crate::{
    account::check_slug,
    catalog::{Mbids, repair_mojibake},
    scrobble::{self, Listen},
};

pub const CLIENT: &str = "import:symfony-2022";
const LISTEN_BATCH: usize = 1_000;
/// Listens within this many seconds of one already there count as "close".
const CLOSE_SECS: i64 = 240;

/// The old database as far as the import needs it.
#[derive(Debug, Default)]
pub struct Source {
    pub profiles: Vec<OldProfile>,
    pub listens: Vec<OldListen>,
}

#[derive(Debug, Clone)]
pub struct OldProfile {
    pub id: i64,
    /// `usr.name` of the account it belongs to.
    pub user: String,
    pub name: String,
    pub is_public: bool,
}

#[derive(Debug, Clone)]
pub struct OldListen {
    pub profile_id: i64,
    /// Unix seconds.
    pub timestamp: i64,
    pub artist: Option<String>,
    pub track: Option<String>,
    pub album: Option<String>,
    /// Seconds, in some rows milliseconds (see [`duration_ms`]).
    pub length: Option<i64>,
    pub track_number: Option<i64>,
}

pub async fn read(url: &str) -> anyhow::Result<Source> {
    let old = PgPool::connect(url)
        .await
        .context("connecting to the old database")?;

    let profiles = sqlx::query_as::<_, (i64, String, String, bool)>(
        "SELECT p.id::int8, u.name::text, p.name::text, p.is_public
           FROM profile p JOIN usr u ON u.id = p.usr
          ORDER BY u.id, p.id",
    )
    .fetch_all(&old)
    .await
    .context("reading the profiles (tables profile and usr)")?
    .into_iter()
    .map(|(id, user, name, is_public)| OldProfile {
        id,
        user,
        name,
        is_public,
    })
    .collect();

    #[allow(clippy::type_complexity)]
    let listens = sqlx::query_as::<
        _,
        (
            i64,
            i64,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<i64>,
            Option<i64>,
        ),
    >(
        r#"SELECT profile_id::int8, "timestamp"::int8, artist_name::text, track_title::text,
                  album_title::text, length::int8, tracknumber::int8
             FROM listen
            ORDER BY profile_id, "timestamp""#,
    )
    .fetch_all(&old)
    .await
    .context("reading the listens (table listen)")?
    .into_iter()
    .map(
        |(profile_id, timestamp, artist, track, album, length, track_number)| OldListen {
            profile_id,
            timestamp,
            artist,
            track,
            album,
            length,
            track_number,
        },
    )
    .collect();

    old.close().await;
    Ok(Source { profiles, listens })
}

/// `--profile OLD_USER/OLD_PROFILE=USER/SLUG`: where an old profile goes, when
/// the names do not tell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mapping {
    pub old_user: String,
    pub old_profile: String,
    pub user: String,
    pub slug: String,
}

impl FromStr for Mapping {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let split = |side: &str| {
            side.split_once('/')
                .map(|(a, b)| (a.trim().to_owned(), b.trim().to_owned()))
                .filter(|(a, b)| !a.is_empty() && !b.is_empty())
        };
        let parsed = text
            .split_once('=')
            .and_then(|(old, new)| Some((split(old)?, split(new)?)));
        let Some(((old_user, old_profile), (user, slug))) = parsed else {
            return Err(format!(
                "\"{text}\" is not OLD_USER/OLD_PROFILE=USER/SLUG, e.g. fiona/Arbeit=fiona/work"
            ));
        };
        Ok(Self {
            old_user,
            old_profile,
            user,
            slug,
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct Options {
    pub dry_run: bool,
    /// Only listens from this time on.
    pub since: Option<OffsetDateTime>,
    /// Only listens before this time.
    pub until: Option<OffsetDateTime>,
    pub mappings: Vec<Mapping>,
}

/// Midnight UTC at the start of `day`, for `--since` and `--until`.
pub fn day_start(day: Date) -> OffsetDateTime {
    day.with_time(Time::MIDNIGHT).assume_utc()
}

/// Where the listens of an old profile go.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// A profile that exists: its id and `user/slug`.
    Profile(i64, String),
    /// A profile of this account that the import creates: account id, user, slug.
    NewProfile(i64, String, String),
    /// The new musicbanana has no account of the name.
    NoAccount,
}

impl fmt::Display for Target {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Profile(_, name) => write!(f, "{name}"),
            Self::NewProfile(_, user, slug) => write!(f, "{user}/{slug} (new profile)"),
            Self::NoAccount => write!(f, "nowhere, no account of that name"),
        }
    }
}

/// How the listens of one old profile line up with those already there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileReport {
    /// `user/profile` in the old database.
    pub old: String,
    pub target: Target,
    /// Old listens in the chosen time range.
    pub listens: usize,
    /// Of these, those with a listen at the same second already.
    pub same_second: usize,
    /// Those with one less than four minutes away (but not at the same second).
    pub close: usize,
    /// Those with one exactly one or two hours away and none closer: a sign
    /// that one of the two sides stored local time.
    pub hours_off: usize,
    /// Listens without artist or title, left out.
    pub incomplete: usize,
    /// Stored as new listens (0 in a dry run).
    pub imported: u64,
}

#[derive(Debug, Default)]
pub struct Report {
    pub dry_run: bool,
    pub profiles: Vec<ProfileReport>,
    /// Names whose double-encoded umlauts were repaired.
    pub repaired_names: usize,
}

impl Report {
    pub fn imported(&self) -> u64 {
        self.profiles.iter().map(|p| p.imported).sum()
    }
}

impl fmt::Display for Report {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for p in &self.profiles {
            writeln!(f, "{} -> {}", p.old, p.target)?;
            writeln!(f, "  listens:        {}", p.listens)?;
            writeln!(f, "  same second:    {} (left out)", p.same_second)?;
            writeln!(
                f,
                "  < 4 min apart:  {} (left out if the same track)",
                p.close
            )?;
            writeln!(
                f,
                "  1 or 2 h off:   {} (time zone? see docs/import-symfony.md)",
                p.hours_off
            )?;
            if p.incomplete > 0 {
                writeln!(f, "  incomplete:     {} (no artist or title)", p.incomplete)?;
            }
            if !self.dry_run {
                writeln!(f, "  imported:       {}", p.imported)?;
            }
        }
        writeln!(f, "repaired names:   {}", self.repaired_names)?;
        if self.dry_run {
            write!(f, "dry run, nothing written")
        } else {
            write!(f, "imported:         {}", self.imported())
        }
    }
}

pub async fn import(db: &PgPool, source: &Source, options: &Options) -> anyhow::Result<Report> {
    let mut report = Report {
        dry_run: options.dry_run,
        ..Report::default()
    };

    for m in &options.mappings {
        if !source
            .profiles
            .iter()
            .any(|p| p.user == m.old_user && p.name == m.old_profile)
        {
            bail!(
                "the old database has no profile {}/{}",
                m.old_user,
                m.old_profile
            );
        }
    }

    let mut by_profile: HashMap<i64, Vec<&OldListen>> = HashMap::new();
    for listen in &source.listens {
        let at = OffsetDateTime::from_unix_timestamp(listen.timestamp)?;
        if options.since.is_some_and(|since| at < since)
            || options.until.is_some_and(|until| at >= until)
        {
            continue;
        }
        by_profile
            .entry(listen.profile_id)
            .or_default()
            .push(listen);
    }

    for old in &source.profiles {
        let listens = by_profile.remove(&old.id).unwrap_or_default();
        let siblings = source
            .profiles
            .iter()
            .filter(|p| p.user == old.user)
            .count();
        let target = target(db, old, siblings, &options.mappings).await?;

        let mut converted = Vec::with_capacity(listens.len());
        let mut incomplete = 0;
        for listen in &listens {
            match convert(listen, &mut report.repaired_names)? {
                Some(l) => converted.push(l),
                None => incomplete += 1,
            }
        }

        let existing = match &target {
            Target::Profile(id, _) => existing_times(db, *id).await?,
            _ => Vec::new(),
        };
        let overlap = Overlap::count(&existing, converted.iter().map(|l| l.listened_at));

        let mut imported = 0;
        if !options.dry_run && !converted.is_empty() {
            let profile_id = match &target {
                Target::Profile(id, _) => Some(*id),
                Target::NewProfile(account_id, _, slug) => {
                    Some(create_profile(db, *account_id, slug, old).await?)
                }
                Target::NoAccount => None,
            };
            if let Some(profile_id) = profile_id {
                for batch in converted.chunks(LISTEN_BATCH) {
                    imported += scrobble::record(db, profile_id, batch).await?;
                }
            }
        }

        report.profiles.push(ProfileReport {
            old: format!("{}/{}", old.user, old.name),
            target,
            listens: listens.len(),
            same_second: overlap.same_second,
            close: overlap.close,
            hours_off: overlap.hours_off,
            incomplete,
            imported,
        });
    }

    Ok(report)
}

/// Picks the profile for `old`: the one named by `--profile`, else the profile
/// of the same name (or address) of the account of the same name, else that
/// account's default profile when the old account had only this one profile,
/// else a new profile of the old name.
async fn target(
    db: &PgPool,
    old: &OldProfile,
    siblings: usize,
    mappings: &[Mapping],
) -> anyhow::Result<Target> {
    if let Some(m) = mappings
        .iter()
        .find(|m| m.old_user == old.user && m.old_profile == old.name)
    {
        let id = crate::yourspotify::profile_id(db, &m.user, &m.slug).await?;
        let name = sqlx::query_scalar!(
            r#"SELECT a.username::text || '/' || p.slug::text AS "name!"
                 FROM profile p JOIN account a ON a.id = p.account_id WHERE p.id = $1"#,
            id
        )
        .fetch_one(db)
        .await?;
        return Ok(Target::Profile(id, name));
    }

    let Some(account) = sqlx::query!(
        r#"SELECT id, username::text AS "username!" FROM account WHERE username = $1::text::citext"#,
        old.user
    )
    .fetch_optional(db)
    .await?
    else {
        return Ok(Target::NoAccount);
    };
    let profiles = sqlx::query!(
        r#"SELECT id, slug::text AS "slug!", name FROM profile WHERE account_id = $1 ORDER BY id"#,
        account.id
    )
    .fetch_all(db)
    .await?;

    let slug = slug_for(&old.name, old.id);
    let same_name = profiles
        .iter()
        .find(|p| p.slug == slug || p.name.to_lowercase() == old.name.trim().to_lowercase());
    let only = (siblings == 1)
        .then(|| {
            profiles
                .iter()
                .find(|p| p.slug == "default")
                .or((profiles.len() == 1).then(|| &profiles[0]))
        })
        .flatten();
    Ok(match same_name.or(only) {
        Some(p) => Target::Profile(p.id, format!("{}/{}", account.username, p.slug)),
        None => Target::NewProfile(account.id, account.username, slug),
    })
}

/// The address for a new profile of the old name: "Arbeit" becomes "arbeit",
/// "Zu Hause" "zu-hause". A name that gives no usable address gets "old-<id>".
pub fn slug_for(name: &str, old_id: i64) -> String {
    let mut slug = String::new();
    for c in name.trim().to_lowercase().chars() {
        let c = match c {
            'ä' => "ae",
            'ö' => "oe",
            'ü' => "ue",
            'ß' => "ss",
            c if c.is_ascii_alphanumeric() => {
                slug.push(c);
                continue;
            }
            _ => "-",
        };
        if c == "-" {
            if !slug.is_empty() && !slug.ends_with('-') {
                slug.push('-');
            }
        } else {
            slug.push_str(c);
        }
    }
    let slug = slug.trim_end_matches('-');
    let slug: String = slug.chars().take(32).collect();
    let slug = slug.trim_end_matches('-').to_owned();
    if check_slug(&slug).is_ok() {
        slug
    } else {
        format!("old-{old_id}")
    }
}

async fn create_profile(
    db: &PgPool,
    account_id: i64,
    slug: &str,
    old: &OldProfile,
) -> anyhow::Result<i64> {
    let visibility = if old.is_public { "public" } else { "private" };
    let id = sqlx::query_scalar!(
        "INSERT INTO profile (account_id, slug, name, visibility)
         VALUES ($1, $2, $3, $4::text::visibility)
         ON CONFLICT (account_id, slug) DO UPDATE SET slug = excluded.slug
         RETURNING id",
        account_id,
        slug,
        old.name.trim(),
        visibility,
    )
    .fetch_one(db)
    .await
    .with_context(|| format!("creating the profile {slug}"))?;
    Ok(id)
}

/// The times of the profile's listens, in unix seconds, sorted.
async fn existing_times(db: &PgPool, profile_id: i64) -> sqlx::Result<Vec<i64>> {
    sqlx::query_scalar!(
        r#"SELECT extract(epoch FROM listened_at)::int8 AS "at!"
             FROM listen WHERE profile_id = $1 ORDER BY listened_at"#,
        profile_id
    )
    .fetch_all(db)
    .await
}

#[derive(Debug, Default, PartialEq, Eq)]
struct Overlap {
    same_second: usize,
    close: usize,
    hours_off: usize,
}

impl Overlap {
    fn count(existing: &[i64], times: impl Iterator<Item = OffsetDateTime>) -> Self {
        let has = |t: i64| existing.binary_search(&t).is_ok();
        let nearest = |t: i64| {
            let i = existing.partition_point(|&e| e < t);
            [i.checked_sub(1), Some(i)]
                .into_iter()
                .flatten()
                .filter_map(|i| existing.get(i))
                .map(|&e| (e - t).abs())
                .min()
        };
        let mut overlap = Self::default();
        for t in times.map(OffsetDateTime::unix_timestamp) {
            match nearest(t) {
                Some(0) => overlap.same_second += 1,
                Some(d) if d < CLOSE_SECS => overlap.close += 1,
                _ if [3600, 7200].iter().any(|h| has(t - h) || has(t + h)) => {
                    overlap.hours_off += 1
                }
                _ => {}
            }
        }
        overlap
    }
}

/// The old listen as a scrobble, or None without artist or title.
fn convert(listen: &OldListen, repaired: &mut usize) -> anyhow::Result<Option<Listen>> {
    let mut fix = |s: &Option<String>| -> Option<String> {
        let s = s.as_deref()?.trim();
        if s.is_empty() {
            return None;
        }
        let fixed = repair_mojibake(s);
        if fixed != s {
            *repaired += 1;
        }
        Some(fixed.into_owned())
    };
    let (Some(artist), Some(track)) = (fix(&listen.artist), fix(&listen.track)) else {
        return Ok(None);
    };
    Ok(Some(Listen {
        listened_at: OffsetDateTime::from_unix_timestamp(listen.timestamp)?,
        artist,
        track,
        album: fix(&listen.album),
        track_number: listen
            .track_number
            .filter(|&n| n > 0)
            .and_then(|n| n.try_into().ok()),
        duration_ms: duration_ms(listen.length),
        client: Some(CLIENT.to_owned()),
        mbids: Mbids::default(),
        extra: None,
    }))
}

/// The old `length` in milliseconds. Mostly seconds; values above 20,000 can
/// only be milliseconds (no track runs five and a half hours), as some players
/// sent them.
pub fn duration_ms(length: Option<i64>) -> Option<i32> {
    let length = length.filter(|&l| l > 0)?;
    let ms = if length > 20_000 {
        length
    } else {
        length * 1000
    };
    ms.try_into().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    #[test]
    fn reads_mappings() {
        assert_eq!(
            "fiona/Zu Hause=fiona/home".parse(),
            Ok(Mapping {
                old_user: "fiona".into(),
                old_profile: "Zu Hause".into(),
                user: "fiona".into(),
                slug: "home".into(),
            })
        );
        for wrong in [
            "fiona/Arbeit",
            "fiona=fiona/work",
            "/x=fiona/work",
            "a/b=c/",
        ] {
            assert!(wrong.parse::<Mapping>().is_err(), "{wrong}");
        }
    }

    #[test]
    fn makes_addresses_from_names() {
        assert_eq!(slug_for("Arbeit", 2), "arbeit");
        assert_eq!(slug_for("  Zu Hause!  ", 2), "zu-hause");
        assert_eq!(slug_for("Büro", 2), "buero");
        assert_eq!(slug_for("Artist", 7), "old-7", "taken by the profile pages");
        assert_eq!(slug_for("♫", 8), "old-8");
        assert_eq!(slug_for(&"a".repeat(40), 9).len(), 32);
    }

    #[test]
    fn tells_seconds_from_milliseconds() {
        assert_eq!(duration_ms(Some(321)), Some(321_000));
        assert_eq!(duration_ms(Some(321_000)), Some(321_000));
        assert_eq!(duration_ms(Some(0)), None);
        assert_eq!(duration_ms(None), None);
    }

    #[test]
    fn counts_the_overlap() {
        let existing = [1_000, 2_000, 10_000];
        let at = |t: i64| OffsetDateTime::from_unix_timestamp(t).unwrap();
        let overlap = Overlap::count(
            &existing,
            [1_000, 1_100, 2_239, 2_240, 4_600, 13_600, 50_000]
                .into_iter()
                .map(at),
        );
        assert_eq!(
            overlap,
            Overlap {
                same_second: 1,
                close: 2,
                hours_off: 2,
            }
        );
        assert_eq!(
            day_start(datetime!(2016-05-31 13:00 UTC).date()).unix_timestamp(),
            1_464_652_800
        );
    }
}
