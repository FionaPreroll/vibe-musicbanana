//! The artist, album and track pages of a profile: listens per month over the
//! profile's whole history, first and last listen, phases of heavy listening,
//! and what was heard of it (an artist's albums and tracks, an album's tracks,
//! the albums a track was played from).
//!
//! An entry that was merged into another one shows that other one, so old links
//! keep working; the response carries the id it shows.

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::get,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use time::OffsetDateTime;

use crate::{
    AppError, AppState,
    profiles::{ChartEntry, Profile, find_profile, time_zone},
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/profiles/{username}/{slug}/artists/{id}", get(artist))
        .route("/profiles/{username}/{slug}/releases/{id}", get(release))
        .route(
            "/profiles/{username}/{slug}/recordings/{id}",
            get(recording),
        )
}

#[derive(Deserialize)]
struct Params {
    /// IANA time zone for the month boundaries, e.g. Europe/Berlin. Defaults to UTC.
    tz: Option<String>,
}

#[derive(Serialize)]
struct Page {
    profile: ProfileRef,
    id: i64,
    name: String,
    /// The artist of an album or track.
    #[serde(skip_serializing_if = "Option::is_none")]
    artist: Option<ArtistRef>,
    listens: i64,
    #[serde(with = "time::serde::rfc3339::option")]
    first_listened_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339::option")]
    last_listened_at: Option<OffsetDateTime>,
    /// Every month from the profile's first listen to its last, oldest first,
    /// except stretches of a year or more without any listens in the profile.
    months: Vec<Month>,
    phases: Vec<Phase>,
    /// The albums it was heard from: an artist's, or those a track was played from.
    releases: Vec<ChartEntry>,
    /// The tracks heard of an artist or from an album.
    recordings: Vec<ChartEntry>,
}

#[derive(Serialize)]
struct ProfileRef {
    username: String,
    slug: String,
    name: String,
}

#[derive(Serialize)]
struct ArtistRef {
    id: i64,
    name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
struct Month {
    /// "2009-03"
    month: String,
    listens: i64,
}

/// Months of heavy listening, see [`phases`].
#[derive(Debug, PartialEq, Serialize)]
struct Phase {
    from: String,
    to: String,
    listens: i64,
}

/// The listens a page is about: those of one artist, release or recording.
#[derive(Clone, Copy, Default)]
struct Of {
    artist: Option<i64>,
    release: Option<i64>,
    recording: Option<i64>,
}

/// How many albums and tracks a page lists.
const LIST_LIMIT: i64 = 30;

async fn artist(
    State(state): State<AppState>,
    Path((username, slug, id)): Path<(String, String, i64)>,
    Query(params): Query<Params>,
) -> Result<Json<Page>, AppError> {
    let profile = find_profile(&state.db, &username, &slug).await?;
    let tz = time_zone(&state.db, params.tz).await?;
    let artist = sqlx::query!(
        "SELECT a.id, a.name FROM artist a
          WHERE a.id = (SELECT coalesce(merged_into, id) FROM artist WHERE id = $1)",
        id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    let subject = Subject {
        id: artist.id,
        name: artist.name,
        artist: None,
        of: Of {
            artist: Some(artist.id),
            ..Of::default()
        },
    };
    Ok(Json(page(&state.db, profile, &tz, subject).await?))
}

async fn release(
    State(state): State<AppState>,
    Path((username, slug, id)): Path<(String, String, i64)>,
    Query(params): Query<Params>,
) -> Result<Json<Page>, AppError> {
    let profile = find_profile(&state.db, &username, &slug).await?;
    let tz = time_zone(&state.db, params.tz).await?;
    let release = sqlx::query!(
        "SELECT r.id, r.title, a.id AS artist_id, a.name AS artist
           FROM release r
           JOIN artist a ON a.id = r.artist_id
          WHERE r.id = (SELECT coalesce(merged_into, id) FROM release WHERE id = $1)",
        id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    let subject = Subject {
        id: release.id,
        name: release.title,
        artist: Some(ArtistRef {
            id: release.artist_id,
            name: release.artist,
        }),
        of: Of {
            release: Some(release.id),
            ..Of::default()
        },
    };
    Ok(Json(page(&state.db, profile, &tz, subject).await?))
}

async fn recording(
    State(state): State<AppState>,
    Path((username, slug, id)): Path<(String, String, i64)>,
    Query(params): Query<Params>,
) -> Result<Json<Page>, AppError> {
    let profile = find_profile(&state.db, &username, &slug).await?;
    let tz = time_zone(&state.db, params.tz).await?;
    let recording = sqlx::query!(
        "SELECT r.id, r.title, a.id AS artist_id, a.name AS artist
           FROM recording r
           JOIN artist a ON a.id = r.artist_id
          WHERE r.id = (SELECT coalesce(merged_into, id) FROM recording WHERE id = $1)",
        id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    let subject = Subject {
        id: recording.id,
        name: recording.title,
        artist: Some(ArtistRef {
            id: recording.artist_id,
            name: recording.artist,
        }),
        of: Of {
            recording: Some(recording.id),
            ..Of::default()
        },
    };
    Ok(Json(page(&state.db, profile, &tz, subject).await?))
}

/// The artist, release or recording a page is about.
struct Subject {
    id: i64,
    name: String,
    artist: Option<ArtistRef>,
    of: Of,
}

/// Counts, months, phases, albums and tracks of the subject's listens in the profile.
async fn page(db: &PgPool, profile: Profile, tz: &str, subject: Subject) -> Result<Page, AppError> {
    let of = subject.of;
    let stats = sqlx::query!(
        r#"SELECT count(*) AS "listens!", min(listened_at) AS first, max(listened_at) AS last
             FROM listen
            WHERE profile_id = $1
              AND ($2::bigint IS NULL OR artist_id = $2)
              AND ($3::bigint IS NULL OR release_id = $3)
              AND ($4::bigint IS NULL OR recording_id = $4)"#,
        profile.id,
        of.artist,
        of.release,
        of.recording,
    )
    .fetch_one(db)
    .await?;

    // The same months on every page of a profile, so the curves line up. A year
    // or more without any listens, e.g. between an import of old data and new
    // scrobbles, is left out.
    let months = sqlx::query_as!(
        Month,
        r#"WITH RECURSIVE heard(month) AS (
               -- The months with listens in the profile, one index lookup each.
               SELECT date_trunc('month', min(listened_at) AT TIME ZONE $5)
                 FROM listen
                WHERE profile_id = $1
               UNION ALL
               SELECT (SELECT date_trunc('month', min(l.listened_at) AT TIME ZONE $5)
                         FROM listen l
                        WHERE l.profile_id = $1
                          AND l.listened_at >= (h.month + interval '1 month') AT TIME ZONE $5)
                 FROM heard h
                WHERE h.month IS NOT NULL
           ), shown AS (
               -- Each of them, and the months up to the next one unless that is a year away.
               SELECT generate_series(
                          month,
                          CASE WHEN next < month + interval '13 months'
                               THEN next - interval '1 month' ELSE month END,
                          interval '1 month') AS month
                 FROM (SELECT month, lead(month) OVER (ORDER BY month) AS next
                         FROM heard
                        WHERE month IS NOT NULL) bounds
           ), counts AS (
               SELECT date_trunc('month', listened_at AT TIME ZONE $5) AS month, count(*) AS listens
                 FROM listen
                WHERE profile_id = $1
                  AND ($2::bigint IS NULL OR artist_id = $2)
                  AND ($3::bigint IS NULL OR release_id = $3)
                  AND ($4::bigint IS NULL OR recording_id = $4)
                GROUP BY 1
           )
           SELECT to_char(s.month, 'YYYY-MM') AS "month!", coalesce(c.listens, 0) AS "listens!"
             FROM shown s
             LEFT JOIN counts c ON c.month = s.month
            ORDER BY s.month"#,
        profile.id,
        of.artist,
        of.release,
        of.recording,
        tz,
    )
    .fetch_all(db)
    .await?;

    // An artist's albums and tracks, an album's tracks, the albums of a track.
    let releases = match of.release {
        None => releases(db, profile.id, of).await?,
        Some(_) => Vec::new(),
    };
    let recordings = match of.recording {
        None => recordings(db, profile.id, of).await?,
        Some(_) => Vec::new(),
    };

    Ok(Page {
        profile: ProfileRef {
            username: profile.username,
            slug: profile.slug,
            name: profile.name,
        },
        id: subject.id,
        name: subject.name,
        artist: subject.artist,
        listens: stats.listens,
        first_listened_at: stats.first,
        last_listened_at: stats.last,
        phases: phases(&months),
        months,
        releases,
        recordings,
    })
}

/// The albums the listens `of` an artist or recording were played from.
async fn releases(db: &PgPool, profile_id: i64, of: Of) -> sqlx::Result<Vec<ChartEntry>> {
    sqlx::query_as!(
        ChartEntry,
        r#"SELECT r.id, r.title AS name, a.name AS "artist?", count(*) AS "listens!"
             FROM listen l
             JOIN release r ON r.id = l.release_id
             JOIN artist a ON a.id = r.artist_id
            WHERE l.profile_id = $1
              AND ($2::bigint IS NULL OR l.artist_id = $2)
              AND ($3::bigint IS NULL OR l.recording_id = $3)
            GROUP BY r.id, a.id
            ORDER BY count(*) DESC, r.title
            LIMIT $4"#,
        profile_id,
        of.artist,
        of.recording,
        LIST_LIMIT,
    )
    .fetch_all(db)
    .await
}

/// The tracks of the listens `of` an artist or release.
async fn recordings(db: &PgPool, profile_id: i64, of: Of) -> sqlx::Result<Vec<ChartEntry>> {
    sqlx::query_as!(
        ChartEntry,
        r#"SELECT r.id, r.title AS name, a.name AS "artist?", count(*) AS "listens!"
             FROM listen l
             JOIN recording r ON r.id = l.recording_id
             JOIN artist a ON a.id = r.artist_id
            WHERE l.profile_id = $1
              AND ($2::bigint IS NULL OR l.artist_id = $2)
              AND ($3::bigint IS NULL OR l.release_id = $3)
            GROUP BY r.id, a.id
            ORDER BY count(*) DESC, r.title
            LIMIT $4"#,
        profile_id,
        of.artist,
        of.release,
        LIST_LIMIT,
    )
    .fetch_all(db)
    .await
}

/// Phases of heavy listening: months with at least twice the average of the
/// shown months from the first listen to the last (and at least three listens),
/// joined across single quieter months in between. Of those, the phases with at
/// least 3 % of all listens count; the five biggest, oldest first.
fn phases(months: &[Month]) -> Vec<Phase> {
    let (Some(first), Some(last)) = (
        months.iter().position(|m| m.listens > 0),
        months.iter().rposition(|m| m.listens > 0),
    ) else {
        return Vec::new();
    };
    let active = &months[first..=last];
    let total: i64 = active.iter().map(|m| m.listens).sum();
    let threshold = (2.0 * total as f64 / active.len() as f64).max(3.0);

    // (first, last) index of each run of heavy months.
    let mut runs: Vec<(usize, usize)> = Vec::new();
    for (i, month) in active.iter().enumerate() {
        if (month.listens as f64) < threshold {
            continue;
        }
        match runs.last_mut() {
            Some((_, end)) if ordinal(&month.month) - ordinal(&active[*end].month) <= 2 => *end = i,
            _ => runs.push((i, i)),
        }
    }

    let mut phases: Vec<(usize, Phase)> = runs
        .into_iter()
        .map(|(start, end)| {
            let listens = active[start..=end].iter().map(|m| m.listens).sum();
            let phase = Phase {
                from: active[start].month.clone(),
                to: active[end].month.clone(),
                listens,
            };
            (start, phase)
        })
        .filter(|(_, phase)| phase.listens * 100 >= total * 3)
        .collect();
    phases.sort_by_key(|(start, phase)| (-phase.listens, *start));
    phases.truncate(5);
    phases.sort_by_key(|(start, _)| *start);
    phases.into_iter().map(|(_, phase)| phase).collect()
}

/// "2009-03" as a number of months, so that following months differ by one.
fn ordinal(month: &str) -> i32 {
    let (year, month) = month.split_once('-').unwrap_or_default();
    year.parse::<i32>().unwrap_or_default() * 12 + month.parse::<i32>().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Months from January 2010 on with the given listens.
    fn months(listens: &[i64]) -> Vec<Month> {
        listens
            .iter()
            .enumerate()
            .map(|(i, &listens)| Month {
                month: format!("{}-{:02}", 2010 + i / 12, i % 12 + 1),
                listens,
            })
            .collect()
    }

    fn phase(from: &str, to: &str, listens: i64) -> Phase {
        Phase {
            from: from.into(),
            to: to.into(),
            listens,
        }
    }

    #[test]
    fn steady_listening_has_no_phases() {
        assert_eq!(phases(&months(&[10; 24])), []);
        assert_eq!(phases(&months(&[0; 24])), []);
        assert_eq!(phases(&[]), []);
    }

    #[test]
    fn heavy_months_make_a_phase_across_a_quiet_one() {
        let found = phases(&months(&[1, 1, 1, 30, 25, 1, 28, 1, 1, 1, 1, 1]));
        assert_eq!(found, [phase("2010-04", "2010-07", 84)]);
    }

    #[test]
    fn the_average_only_counts_from_the_first_listen_to_the_last() {
        // Twelve empty months on either side would halve the average.
        let mut listens = vec![0; 12];
        listens.extend([3, 3, 3, 12, 3, 3]);
        listens.extend([0; 12]);
        assert_eq!(phases(&months(&listens)), [phase("2011-04", "2011-04", 12)]);
    }

    #[test]
    fn phases_do_not_span_a_left_out_gap() {
        // 2010 and 2011, then nothing until October 2026.
        let mut listens = vec![1; 24];
        listens[23] = 30;
        let mut months = months(&listens);
        months.push(Month {
            month: "2026-10".into(),
            listens: 25,
        });
        assert_eq!(
            phases(&months),
            [
                phase("2011-12", "2011-12", 30),
                phase("2026-10", "2026-10", 25)
            ]
        );
    }

    #[test]
    fn a_few_listens_are_no_phase() {
        assert_eq!(phases(&months(&[1, 0, 2, 0, 0, 1, 0, 0, 0, 0])), []);
    }

    #[test]
    fn small_phases_drop_out() {
        // 9 listens are more than twice the average, but not 3 % of all listens.
        let mut listens = vec![300];
        listens.extend([1; 98]);
        listens.push(9);
        assert_eq!(
            phases(&months(&listens)),
            [phase("2010-01", "2010-01", 300)]
        );
    }

    #[test]
    fn at_most_the_five_biggest_phases_stay() {
        // Six bursts of 40 to 90 listens two years apart, one listen a month in between.
        let mut listens = Vec::new();
        for burst in [40, 90, 50, 80, 60, 70] {
            listens.push(burst);
            listens.extend([1; 23]);
        }
        let found = phases(&months(&listens));
        let sizes: Vec<i64> = found.iter().map(|p| p.listens).collect();
        assert_eq!(sizes, [90, 50, 80, 60, 70]);
        assert_eq!(found[0].from, "2012-01");
    }
}
