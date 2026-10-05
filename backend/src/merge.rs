//! Merging catalog entries that are the same under different spellings
//! ("Die Aerzte" and "Die Ärzte"), and finding candidates for it.
//!
//! A merge leaves the raw strings of the listens alone. It points the listens and
//! spellings (alias rows) of one entry at the other one and marks the first as
//! `merged_into` the second, so later scrobbles with the old spelling count for
//! the remaining entry too.
//!
//! Each merge is written down with every row it changed, so that [`undo`] can
//! take it back (see migrations/0008_merge_undo.sql).

use std::{
    cmp::Reverse,
    collections::{HashMap, HashSet},
    fmt,
};

use anyhow::{Context, bail, ensure};
use sqlx::{PgConnection, PgPool};
use unicode_normalization::{UnicodeNormalization, char::is_combining_mark};
use uuid::Uuid;

use crate::catalog::{name_key, without_edition};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Artist,
    Release,
    Recording,
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Kind::Artist => "artist",
            Kind::Release => "release",
            Kind::Recording => "recording",
        })
    }
}

// ---------------------------------------------------------------- suggestions

/// Why an entry looks like another one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Likeness {
    /// Only case, accents, punctuation, spaces, a leading "The" or "&" for "and" differ.
    SameLetters,
    /// One letter more, missing, different, or swapped with its neighbour.
    OneLetter,
    /// The same title apart from a note such as "(Live)", "[Remastered]",
    /// "- Radio Edit" or "feat. …", so possibly another version.
    Version,
}

impl fmt::Display for Likeness {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Likeness::SameLetters => "same letters",
            Likeness::OneLetter => "one letter apart",
            Likeness::Version => "version",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub id: i64,
    pub name: String,
    /// The artist of a release or recording.
    pub artist: Option<String>,
    pub listens: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
    /// The entry to merge: the one with fewer listens, or the version.
    pub from: Entry,
    pub into: Entry,
    pub likeness: Likeness,
}

/// Entries of `kind` with listens that look like another one, most certain first,
/// then by the number of listens a merge would move.
///
/// Releases and recordings are only compared with those of the same artist, so
/// look-alike artists are best merged first; that also merges their releases and
/// recordings with the same title. Two entries that both have a MusicBrainz ID
/// are left out: they are different ones of the same name, or a live version and
/// the original.
pub async fn suggest(db: &PgPool, kind: Kind) -> sqlx::Result<Vec<Suggestion>> {
    // Every entry with the group it is compared within, and whether it has an ID
    // that tells it apart.
    let rows: Vec<(Entry, i64, bool)> = match kind {
        Kind::Artist => sqlx::query!(
            r#"SELECT a.id, a.name, count(*) AS "listens!",
                      EXISTS (SELECT FROM artist_mbid m WHERE m.artist_id = a.id) AS "tagged!"
                 FROM listen l
                 JOIN artist a ON a.id = l.artist_id
                WHERE a.merged_into IS NULL
                GROUP BY a.id
                ORDER BY a.id"#
        )
        .fetch_all(db)
        .await?
        .into_iter()
        .map(|r| {
            let entry = Entry {
                id: r.id,
                name: r.name,
                artist: None,
                listens: r.listens,
            };
            (entry, 0, r.tagged)
        })
        .collect(),
        Kind::Release => sqlx::query!(
            r#"SELECT r.id, r.title, a.id AS artist_id, a.name AS artist, count(*) AS "listens!",
                      EXISTS (SELECT FROM release_mbid m WHERE m.release_id = r.id) AS "tagged!"
                 FROM listen l
                 JOIN release r ON r.id = l.release_id
                 JOIN artist a ON a.id = r.artist_id
                WHERE r.merged_into IS NULL
                GROUP BY r.id, a.id
                ORDER BY r.id"#
        )
        .fetch_all(db)
        .await?
        .into_iter()
        .map(|r| {
            let entry = Entry {
                id: r.id,
                name: r.title,
                artist: Some(r.artist),
                listens: r.listens,
            };
            (entry, r.artist_id, r.tagged)
        })
        .collect(),
        Kind::Recording => sqlx::query!(
            r#"SELECT r.id, r.title, a.id AS artist_id, a.name AS artist, count(*) AS "listens!",
                      EXISTS (SELECT FROM recording_mbid m WHERE m.recording_id = r.id) AS "tagged!"
                 FROM listen l
                 JOIN recording r ON r.id = l.recording_id
                 JOIN artist a ON a.id = r.artist_id
                WHERE r.merged_into IS NULL
                GROUP BY r.id, a.id
                ORDER BY r.id"#
        )
        .fetch_all(db)
        .await?
        .into_iter()
        .map(|r| {
            let entry = Entry {
                id: r.id,
                name: r.title,
                artist: Some(r.artist),
                listens: r.listens,
            };
            (entry, r.artist_id, r.tagged)
        })
        .collect(),
    };

    let mut groups: HashMap<i64, Vec<usize>> = HashMap::new();
    for (i, (_, group, _)) in rows.iter().enumerate() {
        groups.entry(*group).or_default().push(i);
    }
    let mut found = Vec::new();
    for members in groups.values() {
        let names: Vec<&str> = members.iter().map(|&i| rows[i].0.name.as_str()).collect();
        let listens: Vec<i64> = members.iter().map(|&i| rows[i].0.listens).collect();
        for (from, into, likeness) in look_alikes(&names, &listens, kind != Kind::Artist) {
            let (from, into) = (&rows[members[from]], &rows[members[into]]);
            if from.2 && into.2 {
                continue;
            }
            found.push(Suggestion {
                from: from.0.clone(),
                into: into.0.clone(),
                likeness,
            });
        }
    }
    found.sort_by_key(|s| {
        (
            s.likeness,
            Reverse(s.from.listens),
            Reverse(s.into.listens),
            s.from.id,
        )
    });
    Ok(found)
}

/// Look-alikes among `names` as `(from, into, likeness)` indices. `into` is the one
/// with more listens, for versions the title without a note. Every name is `from`
/// at most once, and never also an `into`: chains are followed to their end.
fn look_alikes(names: &[&str], listens: &[i64], titles: bool) -> Vec<(usize, usize, Likeness)> {
    // What a group merges into: most listens, then the oldest entry.
    let best = |group: &[usize]| {
        *group
            .iter()
            .max_by_key(|&&i| (listens[i], Reverse(i)))
            .expect("groups are not empty")
    };
    let folded: Vec<String> = names.iter().map(|name| fold(name)).collect();
    let mut target: HashMap<usize, (usize, Likeness)> = HashMap::new();

    // Same letters: every spelling into the best one.
    let mut by_fold: HashMap<&str, Vec<usize>> = HashMap::new();
    for (i, key) in folded.iter().enumerate() {
        by_fold.entry(key).or_default().push(i);
    }
    let mut heads: Vec<(&str, usize)> = Vec::new();
    for (&key, group) in &by_fold {
        let head = best(group);
        heads.push((key, head));
        for &i in group {
            if i != head {
                target.insert(i, (head, Likeness::SameLetters));
            }
        }
    }
    heads.sort_unstable();

    // One letter apart: the weaker of two heads into the stronger one, and a name
    // close to several others into the one with the most listens.
    let keys: Vec<&str> = heads.iter().map(|&(key, _)| key).collect();
    let mut pairs: Vec<(usize, usize)> = one_edit_pairs(&keys)
        .into_iter()
        .map(|(a, b)| {
            let (a, b) = (heads[a].1, heads[b].1);
            let into = best(&[a, b]);
            (if into == a { b } else { a }, into)
        })
        .collect();
    pairs.sort_unstable_by_key(|&(from, into)| (Reverse(listens[into]), into, from));
    for (from, into) in pairs {
        target.entry(from).or_insert((into, Likeness::OneLetter));
    }

    // Versions: the same title without a note in brackets, after a dash or "feat.".
    if titles {
        let mut by_base: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, name) in names.iter().enumerate() {
            if !target.contains_key(&i) {
                by_base
                    .entry(fold(without_version(name)))
                    .or_default()
                    .push(i);
            }
        }
        for (base, group) in &by_base {
            // The title without a note stays, even with fewer listens.
            let plain: Vec<usize> = group
                .iter()
                .copied()
                .filter(|&i| folded[i] == *base)
                .collect();
            let head = best(if plain.is_empty() { group } else { &plain });
            for &i in group {
                if i != head {
                    target.insert(i, (head, Likeness::Version));
                }
            }
        }
    }

    // a → b → c becomes a → c, with the less certain reason.
    let mut out: Vec<_> = target
        .iter()
        .map(|(&from, &(mut into, mut likeness))| {
            while let Some(&(next, reason)) = target.get(&into) {
                into = next;
                likeness = likeness.max(reason);
            }
            (from, into, likeness)
        })
        .collect();
    out.sort_unstable();
    out
}

/// A name without what spellings of it tend to differ in: case, accents,
/// punctuation, spaces, a leading "The", and "&" or "+" for "and".
/// "The Beatles" → "beatles", "Guns N' Roses" → "gunsnroses", "Björk" → "bjork".
pub fn fold(name: &str) -> String {
    let mut words = Vec::new();
    let mut word = String::new();
    for c in name
        .nfkd()
        .filter(|&c| !is_combining_mark(c))
        .flat_map(char::to_lowercase)
    {
        match c {
            // Letters that do not decompose into a base letter and an accent.
            'ß' => word.push_str("ss"),
            'æ' => word.push_str("ae"),
            'œ' => word.push_str("oe"),
            'ø' => word.push('o'),
            'ł' => word.push('l'),
            'đ' | 'ð' => word.push('d'),
            'þ' => word.push_str("th"),
            c if c.is_alphanumeric() => word.push(c),
            _ => {
                words.push(std::mem::take(&mut word));
                if c == '&' || c == '+' {
                    words.push("and".to_owned());
                }
            }
        }
    }
    words.push(word);
    words.retain(|w| !w.is_empty());
    if words.len() > 1 && words[0] == "the" {
        words.remove(0);
    }
    match words.concat() {
        // Names without letters ("!!!") only match themselves.
        folded if folded.is_empty() => name_key(name),
        folded => folded,
    }
}

/// A title without a version note at its end: "Song (Live)", "Song [2011 Remaster]",
/// "Song - Radio Edit" and "Song feat. Someone" all become "Song".
fn without_version(title: &str) -> &str {
    let mut title = title.trim();
    let mut dash = false;
    loop {
        let shorter = if let Some(s) = strip_brackets(title) {
            s
        } else if !dash && let Some(s) = strip_dash(title) {
            // Only one: "Part 1 - The Beginning - Live" is "Part 1 - The Beginning".
            dash = true;
            s
        } else if let Some(s) = strip_featuring(title) {
            s
        } else {
            return title;
        };
        match shorter.trim_end() {
            "" => return title,
            s => title = s,
        }
    }
}

/// "Song (Live)" → "Song ", also for [] and nested brackets.
fn strip_brackets(title: &str) -> Option<&str> {
    let (open, close) = match title.chars().last()? {
        ')' => ('(', ')'),
        ']' => ('[', ']'),
        _ => return None,
    };
    let mut depth = 0;
    for (i, c) in title.char_indices().rev() {
        if c == close {
            depth += 1;
        } else if c == open {
            depth -= 1;
            if depth == 0 {
                return Some(&title[..i]);
            }
        }
    }
    None
}

fn strip_dash(title: &str) -> Option<&str> {
    [" - ", " – ", " — "]
        .iter()
        .filter_map(|dash| title.rfind(dash))
        .max()
        .map(|i| &title[..i])
}

fn strip_featuring(title: &str) -> Option<&str> {
    // ASCII lowercase keeps the byte offsets.
    let lower = title.to_ascii_lowercase();
    [" feat. ", " feat ", " ft. ", " featuring "]
        .iter()
        .filter_map(|f| lower.find(f))
        .min()
        .map(|i| &title[..i])
}

/// Pairs of `keys` (by index) that are one edit apart. Short keys are left out,
/// where one letter more often makes another word than a typo ("blur", "blue"),
/// and so are keys with different digits ("part1", "part2").
fn one_edit_pairs(keys: &[&str]) -> Vec<(usize, usize)> {
    let chars: Vec<Vec<char>> = keys.iter().map(|k| k.chars().collect()).collect();
    // Keys one edit apart have a form in common with at most one letter left out.
    let mut forms: HashMap<Vec<char>, Vec<usize>> = HashMap::new();
    for (i, key) in chars.iter().enumerate() {
        if key.len() < 5 {
            continue;
        }
        let mut own = HashSet::from([key.clone()]);
        for skip in 0..key.len() {
            let mut form = key.clone();
            form.remove(skip);
            own.insert(form);
        }
        for form in own {
            forms.entry(form).or_default().push(i);
        }
    }
    let digits = |key: &[char]| key.iter().filter(|c| c.is_numeric()).collect::<String>();
    let mut pairs: Vec<(usize, usize)> = forms
        .values()
        .flat_map(|ids| {
            ids.iter()
                .enumerate()
                .flat_map(move |(n, &a)| ids[n + 1..].iter().map(move |&b| (a.min(b), a.max(b))))
        })
        .filter(|&(a, b)| {
            let (a, b) = (&chars[a], &chars[b]);
            a.len().max(b.len()) >= 6 && digits(a) == digits(b) && one_edit_apart(a, b)
        })
        .collect();
    pairs.sort_unstable();
    pairs.dedup();
    pairs
}

/// Whether `b` is `a` with one letter more or less, one letter replaced, or two
/// neighbours swapped.
fn one_edit_apart(a: &[char], b: &[char]) -> bool {
    let (short, long) = if a.len() <= b.len() { (a, b) } else { (b, a) };
    let same = short.iter().zip(long).take_while(|(x, y)| x == y).count();
    match long.len() - short.len() {
        0 if same < short.len() => {
            let next = same + 1;
            short[next..] == long[next..]
                || (next < short.len()
                    && short[same] == long[next]
                    && short[next] == long[same]
                    && short[next + 1..] == long[next + 1..])
        }
        1 => short[same..] == long[same + 1..],
        _ => false,
    }
}

// ---------------------------------------------------------------- merging

/// What a merge changed, or would change in a dry run.
#[derive(Debug)]
pub struct Merged {
    pub kind: Kind,
    pub from: (i64, String),
    pub into: (i64, String),
    pub dry_run: bool,
    /// Listens that count for `into` now.
    pub listens: u64,
    /// Spellings (alias rows) that lead to `into` now.
    pub spellings: u64,
    /// Releases and recordings of a merged artist: moved over to the other artist,
    /// or merged into its entry with the same title.
    pub releases_moved: u64,
    pub releases_merged: u64,
    pub recordings_moved: u64,
    pub recordings_merged: u64,
    /// The number [`undo`] takes it back by; none in a dry run.
    pub op: Option<i64>,
}

impl Merged {
    fn new(kind: Kind, from: &Locked, into: &Locked) -> Self {
        Merged {
            kind,
            from: (from.id, from.name.clone()),
            into: (into.id, into.name.clone()),
            dry_run: false,
            listens: 0,
            spellings: 0,
            releases_moved: 0,
            releases_merged: 0,
            recordings_moved: 0,
            recordings_merged: 0,
            op: None,
        }
    }
}

impl fmt::Display for Merged {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let verb = if self.dry_run {
            "Would merge"
        } else {
            "Merged"
        };
        let (kind, from, into) = (self.kind, &self.from, &self.into);
        writeln!(
            f,
            "{verb} {kind} {} \"{}\" into {} \"{}\".",
            from.0, from.1, into.0, into.1
        )?;
        writeln!(f, "listens:    {}", self.listens)?;
        write!(f, "spellings:  {}", self.spellings)?;
        if kind == Kind::Artist {
            write!(
                f,
                "\nreleases:   {} moved over, {} merged with one of the same title",
                self.releases_moved, self.releases_merged
            )?;
            write!(
                f,
                "\nrecordings: {} moved over, {} merged with one of the same title",
                self.recordings_moved, self.recordings_merged
            )?;
        }
        if self.dry_run {
            write!(f, "\nDry run, nothing was changed.")?;
        } else if let Some(op) = self.op {
            write!(f, "\nUndo with: musicbanana merge undo {op}")?;
        }
        Ok(())
    }
}

/// How [`merge`] goes about it.
#[derive(Debug, Clone, Copy, Default)]
pub struct Options {
    /// Roll the merge back and only report what it would change.
    pub dry_run: bool,
    /// Merge two entries even though their MusicBrainz IDs tell them apart, such
    /// as an artist's other name into the main one, or a recording that
    /// MusicBrainz lists twice.
    pub force: bool,
}

/// Merges the entry `from` of `kind` into `into`, all in one transaction.
pub async fn merge(
    db: &PgPool,
    kind: Kind,
    from: i64,
    into: i64,
    options: Options,
) -> anyhow::Result<Merged> {
    let mut tx = db.begin().await?;
    let op = start_journal(&mut tx, kind, from, into).await?;
    let merged = match kind {
        Kind::Artist => artist(&mut tx, from, into, options.force).await?,
        Kind::Release => release(&mut tx, from, into, options.force).await?,
        Kind::Recording => recording(&mut tx, from, into, options.force).await?,
    };
    end_journal(&mut tx, op, &merged).await?;
    if options.dry_run {
        tx.rollback().await?;
    } else {
        tx.commit().await?;
    }
    Ok(Merged {
        dry_run: options.dry_run,
        op: (!options.dry_run).then_some(op),
        ..merged
    })
}

/// Starts writing down the changes of a merge, see migrations/0008_merge_undo.sql.
async fn start_journal(
    conn: &mut PgConnection,
    kind: Kind,
    from: i64,
    into: i64,
) -> sqlx::Result<i64> {
    let op = sqlx::query_scalar!(
        "INSERT INTO merge_op (kind, from_id, into_id, from_name, into_name)
         VALUES ($1, $2, $3, '', '') RETURNING id",
        kind.to_string(),
        from,
        into,
    )
    .fetch_one(&mut *conn)
    .await?;
    sqlx::query_scalar!(
        "SELECT set_config('musicbanana.merge_op', $1, true)",
        op.to_string()
    )
    .fetch_one(&mut *conn)
    .await?;
    Ok(op)
}

/// Stops writing down changes and names the merged entries in the log.
async fn end_journal(conn: &mut PgConnection, op: i64, merged: &Merged) -> sqlx::Result<()> {
    sqlx::query_scalar!("SELECT set_config('musicbanana.merge_op', '', true)")
        .fetch_one(&mut *conn)
        .await?;
    sqlx::query!(
        "UPDATE merge_op SET from_name = $2, into_name = $3 WHERE id = $1",
        op,
        merged.from.1,
        merged.into.1,
    )
    .execute(&mut *conn)
    .await?;
    Ok(())
}

// ---------------------------------------------------------------- undoing

/// A merge as the log lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeOp {
    pub id: i64,
    pub kind: String,
    pub from: (i64, String),
    pub into: (i64, String),
    pub merged_at: time::OffsetDateTime,
    pub undone_at: Option<time::OffsetDateTime>,
}

impl fmt::Display for MergeOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let day = |t: time::OffsetDateTime| {
            let (y, m, d) = t.to_calendar_date();
            format!(
                "{y}-{:02}-{d:02} {:02}:{:02}",
                m as u8,
                t.hour(),
                t.minute()
            )
        };
        write!(
            f,
            "{:>5}  {}  {} {} \"{}\" into {} \"{}\"",
            self.id,
            day(self.merged_at),
            self.kind,
            self.from.0,
            self.from.1,
            self.into.0,
            self.into.1
        )?;
        if let Some(undone) = self.undone_at {
            write!(f, "  (undone {})", day(undone))?;
        }
        Ok(())
    }
}

/// The latest merges, newest first.
pub async fn log(db: &PgPool, limit: i64) -> sqlx::Result<Vec<MergeOp>> {
    let rows = sqlx::query!(
        "SELECT id, kind, from_id, from_name, into_id, into_name, merged_at, undone_at
           FROM merge_op ORDER BY id DESC LIMIT $1",
        limit
    )
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| MergeOp {
            id: r.id,
            kind: r.kind,
            from: (r.from_id, r.from_name),
            into: (r.into_id, r.into_name),
            merged_at: r.merged_at,
            undone_at: r.undone_at,
        })
        .collect())
}

/// What [`undo`] did.
#[derive(Debug)]
pub struct Undone {
    pub op: MergeOp,
    /// Changed rows put back as they were before the merge.
    pub restored: i64,
    /// Rows that have changed again since and kept their current values.
    pub kept: i64,
    pub dry_run: bool,
}

impl fmt::Display for Undone {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (kind, from, into) = (&self.op.kind, &self.op.from, &self.op.into);
        let verb = if self.dry_run { "Would undo" } else { "Undid" };
        write!(
            f,
            "{verb} merge {}: {kind} {} \"{}\" is apart from {} \"{}\" again.\n\
             {} changes put back",
            self.op.id, from.0, from.1, into.0, into.1, self.restored
        )?;
        if self.kept > 0 {
            write!(
                f,
                ", {} left as they are now, as they have changed since",
                self.kept
            )?;
        }
        write!(f, ".")?;
        if self.dry_run {
            write!(f, "\nDry run, nothing was changed.")?;
        }
        Ok(())
    }
}

/// Takes merge `op` back: the listens, spellings and IDs it moved go back to the
/// merged entry, which stands on its own again. Merges after it that changed the
/// same entries have to be undone first.
pub async fn undo(db: &PgPool, op: i64, dry_run: bool) -> anyhow::Result<Undone> {
    let mut tx = db.begin().await?;
    let row = sqlx::query!(
        "SELECT id, kind, from_id, from_name, into_id, into_name, merged_at, undone_at
           FROM merge_op WHERE id = $1",
        op
    )
    .fetch_optional(&mut *tx)
    .await?
    .with_context(|| format!("there is no merge {op}; `musicbanana merge log` lists them"))?;
    if let Some(when) = row.undone_at {
        bail!("merge {op} was undone already, on {when}");
    }
    let changes = sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!" FROM merge_change WHERE op_id = $1"#,
        op
    )
    .fetch_one(&mut *tx)
    .await?;
    let kept = sqlx::query_scalar!(r#"SELECT undo_merge($1) AS "kept!""#, op)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| match e.as_database_error() {
            Some(db) => anyhow::anyhow!("{}", db.message()),
            None => e.into(),
        })?;
    let undone_at = sqlx::query_scalar!("SELECT undone_at FROM merge_op WHERE id = $1", op)
        .fetch_one(&mut *tx)
        .await?;
    if dry_run {
        tx.rollback().await?;
    } else {
        tx.commit().await?;
    }
    Ok(Undone {
        op: MergeOp {
            id: row.id,
            kind: row.kind,
            from: (row.from_id, row.from_name),
            into: (row.into_id, row.into_name),
            merged_at: row.merged_at,
            undone_at: if dry_run { None } else { undone_at },
        },
        restored: changes - kept,
        kept,
        dry_run,
    })
}

/// An entry about to be merged, locked until the end of the transaction against
/// other merges. The lock is `FOR NO KEY UPDATE`, which scrobbles referencing the
/// entry do not wait for.
struct Locked {
    id: i64,
    name: String,
    merged_into: Option<i64>,
    mbids: Vec<Uuid>,
}

/// Checks that `from` can be merged into `into`. `rows` are the two locked entries.
fn check(
    kind: Kind,
    from: i64,
    into: i64,
    mut rows: Vec<Locked>,
    force: bool,
) -> anyhow::Result<(Locked, Locked)> {
    ensure!(from != into, "{kind} {from} cannot be merged into itself");
    let mut take = |id: i64| {
        let at = rows
            .iter()
            .position(|row| row.id == id)
            .with_context(|| format!("there is no {kind} {id}"))?;
        anyhow::Ok(rows.swap_remove(at))
    };
    let (from, into) = (take(from)?, take(into)?);
    if let Some(other) = from.merged_into {
        bail!("{kind} {} was merged into {other} already", from.id);
    }
    if let Some(other) = into.merged_into {
        bail!(
            "{kind} {} was merged into {other}; merge into that one instead",
            into.id
        );
    }
    ensure!(
        force || !told_apart(&from.mbids, &into.mbids),
        "{kind} {} and {} have different MusicBrainz IDs, so they are not the same; \
         --force merges them anyway",
        from.id,
        into.id
    );
    Ok((from, into))
}

/// Whether MusicBrainz IDs show that two entries are different ones, see
/// [`crate::catalog::Resolver`].
fn told_apart(a: &[Uuid], b: &[Uuid]) -> bool {
    !a.is_empty() && !b.is_empty() && !a.iter().any(|id| b.contains(id))
}

/// Merges an artist with its releases and recordings: those with the same title
/// as one of `into` (in any spelling) are merged into it, the others move over.
/// That goes by their IDs even when `force` merges artists with different ones.
async fn artist(
    conn: &mut PgConnection,
    from: i64,
    into: i64,
    force: bool,
) -> anyhow::Result<Merged> {
    let rows = sqlx::query_as!(
        Locked,
        r#"SELECT a.id, a.name, a.merged_into,
                  array(SELECT m.mbid FROM artist_mbid m WHERE m.artist_id = a.id) AS "mbids!"
             FROM artist a
            WHERE a.id IN ($1, $2)
            ORDER BY a.id
              FOR NO KEY UPDATE"#,
        from,
        into
    )
    .fetch_all(&mut *conn)
    .await?;
    let (from, into) = check(Kind::Artist, from, into, rows, force)?;
    let mut merged = Merged::new(Kind::Artist, &from, &into);

    let mut theirs = releases_of(conn, into.id).await?;
    for mine in releases_of(conn, from.id).await? {
        if let Some(same) = counterpart(&mut theirs, &mine) {
            release(conn, mine.id, same, false).await?;
            merged.releases_merged += 1;
        } else {
            sqlx::query!(
                "UPDATE release SET artist_id = $2 WHERE id = $1",
                mine.id,
                into.id
            )
            .execute(&mut *conn)
            .await?;
            merged.releases_moved += 1;
            theirs.push(mine);
        }
    }

    let mut theirs = recordings_of(conn, into.id).await?;
    for mine in recordings_of(conn, from.id).await? {
        if let Some(same) = counterpart(&mut theirs, &mine) {
            recording(conn, mine.id, same, false).await?;
            merged.recordings_merged += 1;
        } else {
            sqlx::query!(
                "UPDATE recording SET artist_id = $2 WHERE id = $1",
                mine.id,
                into.id
            )
            .execute(&mut *conn)
            .await?;
            merged.recordings_moved += 1;
            theirs.push(mine);
        }
    }

    merged.spellings = sqlx::query!(
        "UPDATE artist_alias SET artist_id = $2 WHERE artist_id = $1",
        from.id,
        into.id
    )
    .execute(&mut *conn)
    .await?
    .rows_affected();
    sqlx::query!(
        "UPDATE artist_mbid SET artist_id = $2 WHERE artist_id = $1",
        from.id,
        into.id
    )
    .execute(&mut *conn)
    .await?;
    // Titles and their IDs are looked up per artist, so those that came with the
    // old artist spellings go over too. Where `into` has one already, it stays.
    sqlx::query!(
        "WITH moved AS (DELETE FROM release_alias WHERE artist_id = $1
                        RETURNING title_key, release_id)
         INSERT INTO release_alias (artist_id, title_key, release_id)
         SELECT $2, title_key, release_id FROM moved
         ON CONFLICT (artist_id, title_key) DO NOTHING",
        from.id,
        into.id
    )
    .execute(&mut *conn)
    .await?;
    sqlx::query!(
        "WITH moved AS (DELETE FROM recording_alias WHERE artist_id = $1
                        RETURNING title_key, recording_id)
         INSERT INTO recording_alias (artist_id, title_key, recording_id)
         SELECT $2, title_key, recording_id FROM moved
         ON CONFLICT (artist_id, title_key) DO NOTHING",
        from.id,
        into.id
    )
    .execute(&mut *conn)
    .await?;
    sqlx::query!(
        "WITH moved AS (DELETE FROM release_mbid WHERE artist_id = $1
                        RETURNING mbid, release_id)
         INSERT INTO release_mbid (artist_id, mbid, release_id)
         SELECT $2, mbid, release_id FROM moved
         ON CONFLICT (artist_id, mbid) DO NOTHING",
        from.id,
        into.id
    )
    .execute(&mut *conn)
    .await?;
    sqlx::query!(
        "WITH moved AS (DELETE FROM recording_mbid WHERE artist_id = $1
                        RETURNING mbid, recording_id)
         INSERT INTO recording_mbid (artist_id, mbid, recording_id)
         SELECT $2, mbid, recording_id FROM moved
         ON CONFLICT (artist_id, mbid) DO NOTHING",
        from.id,
        into.id
    )
    .execute(&mut *conn)
    .await?;

    merged.listens = sqlx::query!(
        "UPDATE listen SET artist_id = $2 WHERE artist_id = $1",
        from.id,
        into.id
    )
    .execute(&mut *conn)
    .await?
    .rows_affected();

    sqlx::query!(
        "UPDATE artist SET merged_into = $2 WHERE merged_into = $1",
        from.id,
        into.id
    )
    .execute(&mut *conn)
    .await?;
    sqlx::query!(
        "UPDATE artist SET merged_into = $2 WHERE id = $1",
        from.id,
        into.id
    )
    .execute(&mut *conn)
    .await?;
    // Keep what only the merged entry knew.
    sqlx::query!(
        "UPDATE artist a SET sort_name = COALESCE(a.sort_name, f.sort_name)
           FROM artist f
          WHERE a.id = $2 AND f.id = $1",
        from.id,
        into.id
    )
    .execute(&mut *conn)
    .await?;
    Ok(merged)
}

/// A release or recording with every key its title is known under, the title's
/// own first, and its MusicBrainz IDs.
struct Titled {
    id: i64,
    keys: Vec<String>,
    mbids: Vec<Uuid>,
}

impl Titled {
    fn new(id: i64, title: &str, keys: Vec<String>, mbids: Vec<Uuid>) -> Self {
        let mut all = vec![name_key(title)];
        all.extend(keys);
        Titled {
            id,
            keys: all,
            mbids,
        }
    }
}

/// The entry among `theirs`, oldest first, that `mine` is merged into when their
/// artists are: the one with an ID in common, or else the oldest known under a
/// title of `mine` (its own first) unless their IDs tell them apart. It takes on
/// the titles and IDs of `mine`.
fn counterpart(theirs: &mut [Titled], mine: &Titled) -> Option<i64> {
    let at = theirs
        .iter()
        .position(|t| t.mbids.iter().any(|id| mine.mbids.contains(id)))
        .or_else(|| {
            mine.keys.iter().find_map(|key| {
                theirs
                    .iter()
                    .position(|t| t.keys.contains(key) && !told_apart(&t.mbids, &mine.mbids))
            })
        })?;
    let same = &mut theirs[at];
    same.keys.extend(mine.keys.iter().cloned());
    same.mbids.extend(&mine.mbids);
    Some(same.id)
}

async fn releases_of(conn: &mut PgConnection, artist_id: i64) -> sqlx::Result<Vec<Titled>> {
    let rows = sqlx::query!(
        r#"SELECT r.id, r.title,
                  array(SELECT x.title_key FROM release_alias x WHERE x.release_id = r.id) AS "keys!",
                  array(SELECT DISTINCT m.mbid FROM release_mbid m WHERE m.release_id = r.id) AS "mbids!"
             FROM release r
            WHERE r.artist_id = $1 AND r.merged_into IS NULL
            ORDER BY r.id"#,
        artist_id
    )
    .fetch_all(&mut *conn)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| Titled::new(r.id, &r.title, r.keys, r.mbids))
        .collect())
}

async fn recordings_of(conn: &mut PgConnection, artist_id: i64) -> sqlx::Result<Vec<Titled>> {
    let rows = sqlx::query!(
        r#"SELECT r.id, r.title,
                  array(SELECT x.title_key FROM recording_alias x WHERE x.recording_id = r.id) AS "keys!",
                  array(SELECT DISTINCT m.mbid FROM recording_mbid m WHERE m.recording_id = r.id) AS "mbids!"
             FROM recording r
            WHERE r.artist_id = $1 AND r.merged_into IS NULL
            ORDER BY r.id"#,
        artist_id
    )
    .fetch_all(&mut *conn)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| Titled::new(r.id, &r.title, r.keys, r.mbids))
        .collect())
}

async fn release(
    conn: &mut PgConnection,
    from: i64,
    into: i64,
    force: bool,
) -> anyhow::Result<Merged> {
    let rows = sqlx::query_as!(
        Locked,
        r#"SELECT r.id, r.title AS name, r.merged_into,
                  array(SELECT DISTINCT m.mbid FROM release_mbid m WHERE m.release_id = r.id) AS "mbids!"
             FROM release r
            WHERE r.id IN ($1, $2)
            ORDER BY r.id
              FOR NO KEY UPDATE"#,
        from,
        into
    )
    .fetch_all(&mut *conn)
    .await?;
    let (from, into) = check(Kind::Release, from, into, rows, force)?;
    let mut merged = Merged::new(Kind::Release, &from, &into);
    merged.listens = sqlx::query!(
        "UPDATE listen SET release_id = $2 WHERE release_id = $1",
        from.id,
        into.id
    )
    .execute(&mut *conn)
    .await?
    .rows_affected();
    merged.spellings = sqlx::query!(
        "UPDATE release_alias SET release_id = $2 WHERE release_id = $1",
        from.id,
        into.id
    )
    .execute(&mut *conn)
    .await?
    .rows_affected();
    sqlx::query!(
        "UPDATE release_mbid SET release_id = $2 WHERE release_id = $1",
        from.id,
        into.id
    )
    .execute(&mut *conn)
    .await?;
    sqlx::query!(
        "UPDATE release SET merged_into = $2 WHERE merged_into = $1",
        from.id,
        into.id
    )
    .execute(&mut *conn)
    .await?;
    sqlx::query!(
        "UPDATE release SET merged_into = $2 WHERE id = $1",
        from.id,
        into.id
    )
    .execute(&mut *conn)
    .await?;
    sqlx::query!(
        "UPDATE release r SET year = COALESCE(r.year, f.year)
           FROM release f
          WHERE r.id = $2 AND f.id = $1",
        from.id,
        into.id
    )
    .execute(&mut *conn)
    .await?;
    Ok(merged)
}

async fn recording(
    conn: &mut PgConnection,
    from: i64,
    into: i64,
    force: bool,
) -> anyhow::Result<Merged> {
    let rows = sqlx::query_as!(
        Locked,
        r#"SELECT r.id, r.title AS name, r.merged_into,
                  array(SELECT DISTINCT m.mbid FROM recording_mbid m WHERE m.recording_id = r.id) AS "mbids!"
             FROM recording r
            WHERE r.id IN ($1, $2)
            ORDER BY r.id
              FOR NO KEY UPDATE"#,
        from,
        into
    )
    .fetch_all(&mut *conn)
    .await?;
    let (from, into) = check(Kind::Recording, from, into, rows, force)?;
    let mut merged = Merged::new(Kind::Recording, &from, &into);
    merged.listens = sqlx::query!(
        "UPDATE listen SET recording_id = $2 WHERE recording_id = $1",
        from.id,
        into.id
    )
    .execute(&mut *conn)
    .await?
    .rows_affected();
    merged.spellings = sqlx::query!(
        "UPDATE recording_alias SET recording_id = $2 WHERE recording_id = $1",
        from.id,
        into.id
    )
    .execute(&mut *conn)
    .await?
    .rows_affected();
    sqlx::query!(
        "UPDATE recording_mbid SET recording_id = $2 WHERE recording_id = $1",
        from.id,
        into.id
    )
    .execute(&mut *conn)
    .await?;
    sqlx::query!(
        "UPDATE recording SET merged_into = $2 WHERE merged_into = $1",
        from.id,
        into.id
    )
    .execute(&mut *conn)
    .await?;
    sqlx::query!(
        "UPDATE recording SET merged_into = $2 WHERE id = $1",
        from.id,
        into.id
    )
    .execute(&mut *conn)
    .await?;
    sqlx::query!(
        "UPDATE recording r SET length_ms = COALESCE(r.length_ms, f.length_ms)
           FROM recording f
          WHERE r.id = $2 AND f.id = $1",
        from.id,
        into.id
    )
    .execute(&mut *conn)
    .await?;
    Ok(merged)
}

// ---------------------------------------------------------------- editions

/// What [`editions`] did with a release or recording whose title names an edition.
#[derive(Debug)]
pub enum Edition {
    /// Merged into the entry of the plain title.
    Merged(Merged),
    /// Renamed to the plain title, which had no entry yet.
    Renamed {
        kind: Kind,
        id: i64,
        from: String,
        to: String,
    },
    /// Left alone, as the entry of the plain title has other MusicBrainz IDs.
    ToldApart {
        kind: Kind,
        id: i64,
        title: String,
        other: (i64, String),
    },
}

impl fmt::Display for Edition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Edition::Merged(m) => write!(
                f,
                "merge {} {} \"{}\" into {} \"{}\": {} {}",
                m.kind,
                m.from.0,
                m.from.1,
                m.into.0,
                m.into.1,
                m.listens,
                if m.listens == 1 { "listen" } else { "listens" }
            )
            .and_then(|()| match m.op {
                Some(op) => write!(f, " (undo with: musicbanana merge undo {op})"),
                None => Ok(()),
            }),
            Edition::Renamed { kind, id, from, to } => {
                write!(f, "rename {kind} {id} \"{from}\" to \"{to}\"")
            }
            Edition::ToldApart {
                kind,
                id,
                title,
                other,
            } => write!(
                f,
                "keep {kind} {id} \"{title}\": {} \"{}\" has other MusicBrainz IDs",
                other.0, other.1
            ),
        }
    }
}

/// Takes the edition notes off the titles of releases and recordings that came
/// in before the catalog left them out (see [`without_edition`]), such as the
/// Spotify plays of an earlier import: each one is merged into the entry of its
/// plain title, or renamed to it where there is none yet. All in one transaction,
/// which a dry run rolls back; each merge goes into the log, so [`undo`] can take
/// it back on its own.
pub async fn editions(db: &PgPool, dry_run: bool) -> anyhow::Result<Vec<Edition>> {
    let mut tx = db.begin().await?;
    let mut done = Vec::new();
    for kind in [Kind::Release, Kind::Recording] {
        for (id, title, artist_id) in with_titles(&mut tx, kind).await? {
            let plain = without_edition(&title);
            if plain == title.trim() {
                continue;
            }
            let key = name_key(plain);
            let other = titled_as(&mut tx, kind, artist_id, &key)
                .await?
                .filter(|other| other.0 != id);
            match other {
                Some((other_id, other_title)) => {
                    let theirs = mbids_of(&mut tx, kind, other_id).await?;
                    let mine = mbids_of(&mut tx, kind, id).await?;
                    if told_apart(&mine, &theirs) {
                        done.push(Edition::ToldApart {
                            kind,
                            id,
                            title,
                            other: (other_id, other_title),
                        });
                        continue;
                    }
                    let op = start_journal(&mut tx, kind, id, other_id).await?;
                    let merged = match kind {
                        Kind::Release => release(&mut tx, id, other_id, false).await?,
                        _ => recording(&mut tx, id, other_id, false).await?,
                    };
                    end_journal(&mut tx, op, &merged).await?;
                    done.push(Edition::Merged(Merged {
                        dry_run,
                        op: (!dry_run).then_some(op),
                        ..merged
                    }));
                }
                None => {
                    match kind {
                        Kind::Release => {
                            sqlx::query!(
                                "WITH renamed AS (UPDATE release SET title = $3 WHERE id = $2)
                                 INSERT INTO release_alias (artist_id, title_key, release_id)
                                 VALUES ($1, $4, $2) ON CONFLICT DO NOTHING",
                                artist_id,
                                id,
                                plain,
                                key,
                            )
                            .execute(&mut *tx)
                            .await?
                        }
                        _ => {
                            sqlx::query!(
                                "WITH renamed AS (UPDATE recording SET title = $3 WHERE id = $2)
                                 INSERT INTO recording_alias (artist_id, title_key, recording_id)
                                 VALUES ($1, $4, $2) ON CONFLICT DO NOTHING",
                                artist_id,
                                id,
                                plain,
                                key,
                            )
                            .execute(&mut *tx)
                            .await?
                        }
                    };
                    done.push(Edition::Renamed {
                        kind,
                        id,
                        to: plain.to_owned(),
                        from: title,
                    });
                }
            }
        }
    }
    if dry_run {
        tx.rollback().await?;
    } else {
        tx.commit().await?;
    }
    Ok(done)
}

/// Every release or recording that was not merged away, with its title and artist.
async fn with_titles(conn: &mut PgConnection, kind: Kind) -> sqlx::Result<Vec<(i64, String, i64)>> {
    Ok(match kind {
        Kind::Release => sqlx::query!(
            "SELECT id, title, artist_id FROM release WHERE merged_into IS NULL ORDER BY id"
        )
        .fetch_all(&mut *conn)
        .await?
        .into_iter()
        .map(|r| (r.id, r.title, r.artist_id))
        .collect(),
        _ => sqlx::query!(
            "SELECT id, title, artist_id FROM recording WHERE merged_into IS NULL ORDER BY id"
        )
        .fetch_all(&mut *conn)
        .await?
        .into_iter()
        .map(|r| (r.id, r.title, r.artist_id))
        .collect(),
    })
}

/// The release or recording of the artist that the title key leads to.
async fn titled_as(
    conn: &mut PgConnection,
    kind: Kind,
    artist_id: i64,
    key: &str,
) -> sqlx::Result<Option<(i64, String)>> {
    Ok(match kind {
        Kind::Release => sqlx::query!(
            "SELECT r.id, r.title FROM release_alias x JOIN release r ON r.id = x.release_id
              WHERE x.artist_id = $1 AND x.title_key = $2 AND r.merged_into IS NULL",
            artist_id,
            key
        )
        .fetch_optional(&mut *conn)
        .await?
        .map(|r| (r.id, r.title)),
        _ => sqlx::query!(
            "SELECT r.id, r.title FROM recording_alias x JOIN recording r ON r.id = x.recording_id
              WHERE x.artist_id = $1 AND x.title_key = $2 AND r.merged_into IS NULL",
            artist_id,
            key
        )
        .fetch_optional(&mut *conn)
        .await?
        .map(|r| (r.id, r.title)),
    })
}

async fn mbids_of(conn: &mut PgConnection, kind: Kind, id: i64) -> sqlx::Result<Vec<Uuid>> {
    match kind {
        Kind::Release => {
            sqlx::query_scalar!("SELECT mbid FROM release_mbid WHERE release_id = $1", id)
                .fetch_all(&mut *conn)
                .await
        }
        _ => {
            sqlx::query_scalar!(
                "SELECT mbid FROM recording_mbid WHERE recording_id = $1",
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

    #[test]
    fn fold_ignores_what_spellings_differ_in() {
        for (a, b) in [
            ("Björk", "Bjork"),
            ("The Beatles", "Beatles"),
            ("Guns N' Roses", "Guns 'n' Roses"),
            ("AC/DC", "ACDC"),
            ("Simon & Garfunkel", "Simon and Garfunkel"),
            ("Florence + the Machine", "Florence and the Machine"),
            ("Sigur Rós", "sigur ros"),
            ("Motörhead", "MOTORHEAD"),
            ("Weiß", "Weiss"),
            ("Jóga", "Joga"),
        ] {
            assert_eq!(fold(a), fold(b), "{a} / {b}");
        }
        assert_eq!(fold("The Beatles"), "beatles");
        assert_eq!(fold("The The"), "the");
        assert_ne!(fold("Die Ärzte"), fold("Die Aerzte"));
        // Without letters, names stay apart.
        assert_ne!(fold("!!!"), fold("???"));
    }

    #[test]
    fn version_notes_come_off() {
        for (title, plain) in [
            ("Song (Live)", "Song"),
            ("Song [2011 Remaster]", "Song"),
            ("Song (Live) [Remastered]", "Song"),
            ("Song (Live (Berlin))", "Song"),
            ("Song - Radio Edit", "Song"),
            ("Song – Live", "Song"),
            ("Song (Live) - Remastered", "Song"),
            ("Song feat. Someone", "Song"),
            ("Song Ft. Someone", "Song"),
            ("Part 1 - The Beginning - Live", "Part 1 - The Beginning"),
            (
                "(What's the Story) Morning Glory?",
                "(What's the Story) Morning Glory?",
            ),
            ("(Untitled)", "(Untitled)"),
            ("Jay-Z", "Jay-Z"),
        ] {
            assert_eq!(without_version(title), plain, "{title}");
        }
    }

    fn chars(s: &str) -> Vec<char> {
        s.chars().collect()
    }

    #[test]
    fn one_edit() {
        for (a, b) in [
            ("ramstein", "rammstein"),
            ("diearzte", "dieaerzte"),
            ("metallica", "metalica"),
            ("nirvana", "nirvnaa"),
            ("abcdef", "xbcdef"),
            ("abcdef", "abcdex"),
            ("abcdef", "abcdefg"),
        ] {
            assert!(one_edit_apart(&chars(a), &chars(b)), "{a} / {b}");
            assert!(one_edit_apart(&chars(b), &chars(a)), "{b} / {a}");
        }
        for (a, b) in [
            ("abcdef", "abcdef"),
            ("abcdef", "abcfed"),
            ("abcdef", "abcdefgh"),
            ("abcdef", "xbcdey"),
        ] {
            assert!(!one_edit_apart(&chars(a), &chars(b)), "{a} / {b}");
        }
    }

    #[test]
    fn one_edit_pairs_skip_short_keys_and_other_numbers() {
        let keys = [
            "blur",
            "blue",
            "chapter1",
            "chapter2",
            "rammstein",
            "ramstein",
        ];
        assert_eq!(one_edit_pairs(&keys), [(4, 5)]);
    }

    /// `(from, into, likeness)` by name.
    fn alikes(entries: &[(&str, i64)], titles: bool) -> Vec<(String, String, Likeness)> {
        let names: Vec<&str> = entries.iter().map(|e| e.0).collect();
        let listens: Vec<i64> = entries.iter().map(|e| e.1).collect();
        look_alikes(&names, &listens, titles)
            .into_iter()
            .map(|(from, into, l)| (names[from].to_owned(), names[into].to_owned(), l))
            .collect()
    }

    fn alike(from: &str, into: &str, likeness: Likeness) -> (String, String, Likeness) {
        (from.to_owned(), into.to_owned(), likeness)
    }

    #[test]
    fn look_alikes_merge_into_the_most_listened() {
        let found = alikes(
            &[
                ("Björk", 410),
                ("Bjork", 3),
                ("BJÖRK!", 5),
                ("Die Ärzte", 2345),
                ("Die Aerzte", 12),
                ("Blur", 40),
                ("Blue", 2),
            ],
            false,
        );
        assert_eq!(
            found,
            [
                alike("Bjork", "Björk", Likeness::SameLetters),
                alike("BJÖRK!", "Björk", Likeness::SameLetters),
                alike("Die Aerzte", "Die Ärzte", Likeness::OneLetter),
            ]
        );
    }

    #[test]
    fn versions_merge_into_the_plain_title() {
        let found = alikes(
            &[
                ("Song (Live)", 50),
                ("Song", 10),
                ("song (live)", 1),
                ("Other Song", 7),
                ("Part 1", 3),
                ("Part 2", 3),
            ],
            true,
        );
        assert_eq!(
            found,
            [
                alike("Song (Live)", "Song", Likeness::Version),
                // Same letters as "Song (Live)", which is a version of "Song".
                alike("song (live)", "Song", Likeness::Version),
            ]
        );
        // Artists have no versions.
        assert_eq!(alikes(&[("Song (Live)", 50), ("Song", 10)], false), []);
    }

    #[test]
    fn chains_end_at_one_entry() {
        // "Ramsteinn" is one letter from "Ramstein" only, which is one from "Rammstein".
        let found = alikes(
            &[("Rammstein", 100), ("Ramstein", 20), ("Ramsteinn", 1)],
            false,
        );
        assert_eq!(
            found,
            [
                alike("Ramstein", "Rammstein", Likeness::OneLetter),
                alike("Ramsteinn", "Rammstein", Likeness::OneLetter),
            ]
        );
    }
}
