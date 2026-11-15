//! Import of the musicbanana-symfony database (2021-2022, PostgreSQL).
//!
//! Unlike the PHP import this one adds to a musicbanana that is in use. The old
//! version showed all profiles of an account together, as the players they came
//! from; here they are sources. So all old profiles of an account go into its
//! default profile, each under its own source ("import:symfony-2022/gonic"; the
//! first profile, called "_", as plain "import:symfony-2022").
//!
//! A play the account already has, in any of its profiles, is left out: a
//! listen at the same second (the PHP import of the same years) or one of the
//! same track a few moments apart (Navidrome, YourSpotify). The same holds
//! between the old profiles. Where such a play came from the PHP import and the
//! old profile has a name, the PHP listen takes that profile's source instead,
//! as the old version knew where it came from. The rest goes in through
//! [`scrobble::record`], like the listens of a scrobbler, which also keeps out
//! what the owner moved to the trash. A second run adds nothing.
//!
//! [`read`] loads the old database, [`import`] matches and writes. With
//! `dry_run` it only counts, including listens a whole hour or two off from one
//! already there, which would mean one of the imports stored local time.

use std::{
    collections::{BTreeMap, HashMap, HashSet, hash_map::Entry},
    fmt,
    str::FromStr,
};

use anyhow::{Context, bail};
use sqlx::PgPool;
use time::{Date, OffsetDateTime, Time};

use crate::{
    catalog::{Mbids, name_key, repair_mojibake},
    import_php,
    scrobble::{self, Listen},
};

pub const CLIENT: &str = "import:symfony-2022";
const LISTEN_BATCH: usize = 1_000;
/// A listen of the same track closer than half its length, at most this many
/// seconds, to one already there is the same play (as in [`scrobble::record`]).
const MAX_GAP_SECS: f64 = 240.0;
/// The gap when the length is unknown.
const UNKNOWN_GAP_SECS: f64 = 15.0;

/// The old database as far as the import needs it.
#[derive(Debug, Default)]
pub struct Source {
    pub profiles: Vec<OldProfile>,
    pub listens: Vec<OldListen>,
}

#[derive(Debug, Clone)]
pub struct OldProfile {
    pub id: i64,
    /// `users.name` of the account it belongs to.
    pub user: String,
    pub name: String,
    pub is_public: bool,
}

impl OldProfile {
    /// The source its listens get: "_" is the old version itself, any other
    /// profile was a player or an import of its own.
    pub fn client(&self) -> String {
        match self.name.trim() {
            "_" => CLIENT.to_owned(),
            name => format!("{CLIENT}/{name}"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct OldListen {
    pub profile_id: i64,
    /// Unix seconds.
    pub timestamp: i64,
    pub artist: Option<String>,
    pub track: Option<String>,
    pub album: Option<String>,
    /// Seconds, in some rows milliseconds, 0 when unknown (see [`duration_ms`]).
    pub length: Option<i64>,
    pub track_number: Option<i64>,
}

pub async fn read(url: &str) -> anyhow::Result<Source> {
    let old = PgPool::connect(url)
        .await
        .context("connecting to the old database")?;

    let profiles = sqlx::query_as::<_, (i64, String, String, bool)>(
        "SELECT p.id::int8, u.name::text, p.name::text, p.is_public
           FROM profile p JOIN users u ON u.id = p.user_id
          ORDER BY u.id, p.id",
    )
    .fetch_all(&old)
    .await
    .context("reading the profiles (tables profile and users)")?
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
             FROM scrobble
            WHERE profile_id IS NOT NULL
            ORDER BY profile_id, "timestamp""#,
    )
    .fetch_all(&old)
    .await
    .context("reading the listens (table scrobble)")?
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

/// `--user OLD_USER=USER`: the account the profiles of an old account go to,
/// when it has another name now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserMapping {
    pub old_user: String,
    pub user: String,
}

impl FromStr for UserMapping {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match text.split_once('=') {
            Some((old, new))
                if !old.trim().is_empty()
                    && !new.trim().is_empty()
                    && !old.contains('/')
                    && !new.contains('/') =>
            {
                Ok(Self {
                    old_user: old.trim().to_owned(),
                    user: new.trim().to_owned(),
                })
            }
            _ => Err(format!("\"{text}\" is not OLD_USER=USER, e.g. fi=fiona")),
        }
    }
}

/// `--skip OLD_USER/OLD_PROFILE`: an old profile to leave out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OldName {
    pub user: String,
    pub profile: String,
}

impl FromStr for OldName {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match text.split_once('/') {
            Some((user, profile)) if !user.trim().is_empty() && !profile.trim().is_empty() => {
                Ok(Self {
                    user: user.trim().to_owned(),
                    profile: profile.trim().to_owned(),
                })
            }
            _ => Err(format!(
                "\"{text}\" is not OLD_USER/OLD_PROFILE, e.g. fiona/Arbeit"
            )),
        }
    }
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
    pub users: Vec<UserMapping>,
    pub skip: Vec<OldName>,
}

/// Midnight UTC at the start of `day`, for `--since` and `--until`.
pub fn day_start(day: Date) -> OffsetDateTime {
    day.with_time(Time::MIDNIGHT).assume_utc()
}

/// Where the listens of an old profile go.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// A profile: its id and `user/slug`.
    Profile(i64, String),
    /// The new musicbanana has no account of the name.
    NoAccount,
    /// Left out with `--skip`.
    Skipped,
}

impl fmt::Display for Target {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Profile(_, name) => write!(f, "{name}"),
            Self::NoAccount => write!(f, "nowhere, no account of that name"),
            Self::Skipped => write!(f, "nowhere, skipped"),
        }
    }
}

/// How the listens of one old profile line up with those already there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileReport {
    /// `user/profile` in the old database.
    pub old: String,
    pub target: Target,
    /// The source its listens get.
    pub source: String,
    /// Old listens in the chosen time range.
    pub listens: usize,
    /// Of these, those with a listen at the same second in any profile of the
    /// account, left out.
    pub same_second: usize,
    /// Those with a listen of the same track a few moments away in any profile
    /// of the account, left out.
    pub same_track: usize,
    /// Listens from the PHP import among those two kinds that take this
    /// profile's source.
    pub resourced: usize,
    /// Those with a listen exactly one or two hours away: a sign that one of
    /// the two sides stored local time. They count as new.
    pub hours_off: usize,
    /// Those that are new.
    pub new: usize,
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
            writeln!(f, "{} -> {} as {}", p.old, p.target, p.source)?;
            writeln!(f, "  listens:        {}", p.listens)?;
            writeln!(f, "  same second:    {} (left out)", p.same_second)?;
            writeln!(
                f,
                "  same track:     {} (moments apart, left out)",
                p.same_track
            )?;
            if p.resourced > 0 {
                writeln!(
                    f,
                    "  PHP import:     {} of these take the source {}",
                    p.resourced, p.source
                )?;
            }
            writeln!(f, "  new:            {}", p.new)?;
            writeln!(
                f,
                "  1 or 2 h off:   {} of the new ones (time zone? see docs/import-symfony.md)",
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

    for u in &options.users {
        if !source.profiles.iter().any(|p| p.user == u.old_user) {
            bail!("the old database has no profiles of {}", u.old_user);
        }
        let exists = sqlx::query_scalar!(
            r#"SELECT EXISTS (SELECT FROM account WHERE username = $1::text::citext) AS "exists!""#,
            u.user
        )
        .fetch_one(db)
        .await?;
        if !exists {
            bail!("there is no account {}", u.user);
        }
    }
    let named = options
        .mappings
        .iter()
        .map(|m| (&m.old_user, &m.old_profile))
        .chain(options.skip.iter().map(|s| (&s.user, &s.profile)));
    for (user, profile) in named {
        if !source
            .profiles
            .iter()
            .any(|p| &p.user == user && &p.name == profile)
        {
            bail!("the old database has no profile {user}/{profile}");
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

    // Per account: its listens in all profiles, and those taken from the old
    // database so far, so that a play in two old profiles counts once as well.
    let mut seen: HashMap<i64, Seen> = HashMap::new();
    for old in &source.profiles {
        let listens = by_profile.remove(&old.id).unwrap_or_default();
        let skipped = options
            .skip
            .iter()
            .any(|s| s.user == old.user && s.profile == old.name);
        let target = if skipped {
            Target::Skipped
        } else {
            let username = options
                .users
                .iter()
                .find(|u| u.old_user == old.user)
                .map_or(old.user.as_str(), |u| u.user.as_str());
            target(db, old, username, &options.mappings).await?
        };
        let client = old.client();

        let mut converted = Vec::with_capacity(listens.len());
        let mut incomplete = 0;
        for listen in &listens {
            match convert(listen, &client, &mut report.repaired_names)? {
                Some(l) => converted.push(l),
                None => incomplete += 1,
            }
        }

        let mut counts = Counts::default();
        let mut accepted = Vec::new();
        let mut resource = Vec::new();
        if let Target::Profile(profile_id, _) = &target {
            let account_id =
                sqlx::query_scalar!("SELECT account_id FROM profile WHERE id = $1", profile_id)
                    .fetch_one(db)
                    .await?;
            let seen = match seen.entry(account_id) {
                Entry::Occupied(e) => e.into_mut(),
                Entry::Vacant(e) => e.insert(Seen::of_account(db, account_id).await?),
            };
            for listen in converted {
                let key = Seen::key(&listen);
                let at = listen.listened_at.unix_timestamp();
                let (verdict, php) = seen.judge(at, &key, gap_secs(listen.duration_ms));
                match verdict {
                    Verdict::SameSecond | Verdict::SameTrack => {
                        if verdict == Verdict::SameSecond {
                            counts.same_second += 1;
                        } else {
                            counts.same_track += 1;
                        }
                        // Only a named profile says more than the PHP import.
                        if let Some(id) = php.filter(|_| client != CLIENT) {
                            seen.claim(id);
                            resource.push(id);
                        }
                    }
                    Verdict::HoursOff | Verdict::New => {
                        counts.hours_off += usize::from(verdict == Verdict::HoursOff);
                        seen.add(at, key, None);
                        accepted.push(listen);
                    }
                }
            }
        }

        let mut imported = 0;
        if let (false, Target::Profile(profile_id, _)) = (options.dry_run, &target) {
            for batch in accepted.chunks(LISTEN_BATCH) {
                imported += scrobble::record(db, *profile_id, batch).await?;
            }
            if !resource.is_empty() {
                sqlx::query!(
                    "UPDATE listen SET client = $2 WHERE id = ANY($1) AND client = $3",
                    &resource,
                    client,
                    import_php::CLIENT,
                )
                .execute(db)
                .await?;
            }
        }

        report.profiles.push(ProfileReport {
            old: format!("{}/{}", old.user, old.name),
            target,
            source: client,
            listens: listens.len(),
            same_second: counts.same_second,
            same_track: counts.same_track,
            resourced: resource.len(),
            hours_off: counts.hours_off,
            new: accepted.len(),
            incomplete,
            imported,
        });
    }

    Ok(report)
}

/// Picks the profile for `old`: the one named by `--profile`, else the default
/// profile of the account `username` (the old account's name unless `--user`
/// gives another), or its first one if it has none called default.
async fn target(
    db: &PgPool,
    old: &OldProfile,
    username: &str,
    mappings: &[Mapping],
) -> anyhow::Result<Target> {
    let found = if let Some(m) = mappings
        .iter()
        .find(|m| m.old_user == old.user && m.old_profile == old.name)
    {
        let id = crate::yourspotify::profile_id(db, &m.user, &m.slug).await?;
        Some(id)
    } else {
        sqlx::query_scalar!(
            "SELECT p.id FROM profile p JOIN account a ON a.id = p.account_id
              WHERE a.username = $1::text::citext
              ORDER BY p.slug <> 'default', p.id LIMIT 1",
            username
        )
        .fetch_optional(db)
        .await?
    };
    let Some(id) = found else {
        return Ok(Target::NoAccount);
    };
    let name = sqlx::query_scalar!(
        r#"SELECT a.username::text || '/' || p.slug::text AS "name!"
             FROM profile p JOIN account a ON a.id = p.account_id WHERE p.id = $1"#,
        id
    )
    .fetch_one(db)
    .await?;
    Ok(Target::Profile(id, name))
}

#[derive(Debug, Default)]
struct Counts {
    same_second: usize,
    same_track: usize,
    hours_off: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Verdict {
    SameSecond,
    SameTrack,
    HoursOff,
    New,
}

/// Artist and title as the alias tables compare them.
type TrackKey = (String, String);

/// The listens of one account, all its profiles together, as far as telling
/// a second copy of a play goes. Each time keeps the id of its listen if that
/// came from the PHP import and may take another source.
#[derive(Debug, Default)]
struct Seen {
    times: BTreeMap<i64, Option<i64>>,
    tracks: HashMap<TrackKey, BTreeMap<i64, Option<i64>>>,
    claimed: HashSet<i64>,
}

impl Seen {
    async fn of_account(db: &PgPool, account_id: i64) -> sqlx::Result<Self> {
        let rows = sqlx::query!(
            r#"SELECT l.id, extract(epoch FROM l.listened_at)::int8 AS "at!",
                      l.artist_raw, l.track_raw, l.client = $2 AS "php!"
                 FROM listen l JOIN profile p ON p.id = l.profile_id
                WHERE p.account_id = $1"#,
            account_id,
            import_php::CLIENT,
        )
        .fetch_all(db)
        .await?;
        let mut seen = Self::default();
        for row in rows {
            seen.add(
                row.at,
                (name_key(&row.artist_raw), name_key(&row.track_raw)),
                row.php.then_some(row.id),
            );
        }
        Ok(seen)
    }

    fn key(listen: &Listen) -> TrackKey {
        (name_key(&listen.artist), name_key(&listen.track))
    }

    fn add(&mut self, at: i64, key: TrackKey, php: Option<i64>) {
        self.times.entry(at).or_insert(php);
        self.tracks.entry(key).or_default().entry(at).or_insert(php);
    }

    /// The PHP listen `id` takes the source of one old profile, so no other.
    fn claim(&mut self, id: i64) {
        self.claimed.insert(id);
    }

    /// Whether a listen of `key` at `at` is one already there, and the PHP
    /// listen it matches, if any is still free.
    fn judge(&self, at: i64, key: &TrackKey, gap_secs: f64) -> (Verdict, Option<i64>) {
        let free = |php: &Option<i64>| php.filter(|id| !self.claimed.contains(id));
        if let Some(php) = self.times.get(&at) {
            return (Verdict::SameSecond, free(php));
        }
        let reach = gap_secs.ceil() as i64;
        let near = self.tracks.get(key).and_then(|times| {
            times
                .range(at - reach..=at + reach)
                .filter(|&(&t, _)| ((t - at).abs() as f64) < gap_secs)
                .min_by_key(|&(&t, _)| (t - at).abs())
        });
        if let Some((_, php)) = near {
            return (Verdict::SameTrack, free(php));
        }
        if [3600, 7200]
            .iter()
            .any(|h| self.times.contains_key(&(at - h)) || self.times.contains_key(&(at + h)))
        {
            return (Verdict::HoursOff, None);
        }
        (Verdict::New, None)
    }
}

/// How far apart two listens of a track must be to be two plays: half its
/// length, at most four minutes, 15 seconds when the length is unknown.
fn gap_secs(duration_ms: Option<i32>) -> f64 {
    duration_ms.map_or(UNKNOWN_GAP_SECS, |ms| {
        (f64::from(ms) / 2000.0).min(MAX_GAP_SECS)
    })
}

/// The old listen as a scrobble, or None without artist or title.
fn convert(
    listen: &OldListen,
    client: &str,
    repaired: &mut usize,
) -> anyhow::Result<Option<Listen>> {
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
        client: Some(client.to_owned()),
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
        assert_eq!(
            " fi = fiona ".parse(),
            Ok(UserMapping {
                old_user: "fi".into(),
                user: "fiona".into(),
            })
        );
        for wrong in ["fi", "fi=", "fi/_=fiona", "fi=fiona/default"] {
            assert!(wrong.parse::<UserMapping>().is_err(), "{wrong}");
        }
        assert_eq!(
            "fiona/_".parse(),
            Ok(OldName {
                user: "fiona".into(),
                profile: "_".into(),
            })
        );
        for wrong in ["fiona", "fiona/", " /x"] {
            assert!(wrong.parse::<OldName>().is_err(), "{wrong}");
        }
    }

    #[test]
    fn names_sources_after_profiles() {
        let profile = |name: &str| OldProfile {
            id: 1,
            user: "fiona".into(),
            name: name.into(),
            is_public: true,
        };
        assert_eq!(profile("_").client(), "import:symfony-2022");
        assert_eq!(profile("gonic").client(), "import:symfony-2022/gonic");
    }

    #[test]
    fn tells_seconds_from_milliseconds() {
        assert_eq!(duration_ms(Some(321)), Some(321_000));
        assert_eq!(duration_ms(Some(321_000)), Some(321_000));
        assert_eq!(duration_ms(Some(0)), None);
        assert_eq!(duration_ms(None), None);
    }

    #[test]
    fn tells_second_copies_of_plays() {
        let key = |a: &str, t: &str| (name_key(a), name_key(t));
        let mut seen = Seen::default();
        seen.add(1_000, key("Die Ärzte", "Westerland"), Some(7));
        seen.add(2_000, key("Björk", "Hyperballad"), None);
        let westerland = key("die ärzte ", "WESTERLAND");
        assert_eq!(
            seen.judge(1_000, &key("x", "y"), 15.0),
            (Verdict::SameSecond, Some(7))
        );
        assert_eq!(
            seen.judge(1_100, &westerland, 115.0),
            (Verdict::SameTrack, Some(7))
        );
        assert_eq!(seen.judge(1_115, &westerland, 115.0), (Verdict::New, None));
        assert_eq!(
            seen.judge(2_010, &key("Björk", "Hyperballad"), 15.0),
            (Verdict::SameTrack, None)
        );
        assert_eq!(
            seen.judge(1_100, &key("Björk", "Joga"), 115.0),
            (Verdict::New, None)
        );
        assert_eq!(
            seen.judge(4_600, &westerland, 115.0),
            (Verdict::HoursOff, None)
        );
        assert_eq!(
            seen.judge(9_200, &westerland, 115.0),
            (Verdict::HoursOff, None)
        );
        assert_eq!(seen.judge(50_000, &westerland, 115.0), (Verdict::New, None));
        // Once claimed, the PHP listen takes no second source.
        seen.claim(7);
        assert_eq!(
            seen.judge(1_000, &key("x", "y"), 15.0),
            (Verdict::SameSecond, None)
        );

        assert_eq!(gap_secs(None), 15.0);
        assert_eq!(gap_secs(Some(230_000)), 115.0);
        assert_eq!(gap_secs(Some(900_000)), 240.0);
        assert_eq!(
            day_start(datetime!(2016-05-31 13:00 UTC).date()).unix_timestamp(),
            1_464_652_800
        );
    }
}
