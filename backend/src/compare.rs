//! Two profiles side by side: what both of them listen to, how much their
//! listening overlaps, and what each one hears that the other never did.
//!
//! The viewer must be allowed to see both profiles (see [`crate::profiles`]);
//! otherwise the answer is 404, as for an unknown profile.
//!
//! The overlap of two profiles is the share of listening they have in common:
//! each profile's listens of an artist as a share of all its listens in the
//! period, and the smaller of the two shares summed over all artists. It is 1
//! for two profiles that heard the same artists equally often, and 0 for two
//! without a single artist in common. The shared artists, albums and tracks are
//! ranked by what they add to that sum.

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::get,
};
use serde::Serialize;

use crate::{
    AppError, AppState,
    account::Viewer,
    profiles::{Profile, TopParams, find_profile, time_zone},
};

pub fn routes() -> Router<AppState> {
    Router::new().route(
        "/compare/{username}/{slug}/{other}/{other_slug}",
        get(compare),
    )
}

#[derive(Serialize)]
struct Comparison {
    a: Side,
    b: Side,
    /// The overlap of both profiles' artists, from 0 to 1 (see the module docs).
    score: f64,
    /// Artists, albums and tracks both profiles heard in the period.
    shared: SharedCounts,
    artists: Vec<SharedEntry>,
    releases: Vec<SharedEntry>,
    recordings: Vec<SharedEntry>,
}

#[derive(Serialize)]
struct Side {
    username: String,
    slug: String,
    name: String,
    /// Listens in the period.
    listens: i64,
    /// Different artists in the period.
    artists: i64,
    /// Artists of the period the other profile never heard at all.
    only_count: i64,
    /// The most heard of them.
    only: Vec<OnlyEntry>,
}

#[derive(Serialize, Default)]
struct SharedCounts {
    artists: i64,
    releases: i64,
    recordings: i64,
}

#[derive(Serialize)]
struct SharedEntry {
    id: i64,
    name: String,
    /// The artist of a release or recording; missing for artists.
    #[serde(skip_serializing_if = "Option::is_none")]
    artist: Option<String>,
    /// Listens of the first and of the second profile.
    a: i64,
    b: i64,
}

#[derive(Serialize)]
struct OnlyEntry {
    id: i64,
    name: String,
    listens: i64,
}

async fn compare(
    State(state): State<AppState>,
    viewer: Viewer,
    Path((username, slug, other, other_slug)): Path<(String, String, String, String)>,
    Query(params): Query<TopParams>,
) -> Result<Json<Comparison>, AppError> {
    let (from, to) = params.days()?;
    let (a, b) = tokio::try_join!(
        find_profile(&state.db, viewer, &username, &slug),
        find_profile(&state.db, viewer, &other, &other_slug),
    )?;
    if a.id == b.id {
        return Err(AppError::BadRequest(
            "a profile cannot be compared with itself".into(),
        ));
    }
    let tz = time_zone(&state.db, params.tz.clone()).await?;
    let limit = params.limit.unwrap_or(10).clamp(1, 100);
    let source = params.source.as_deref();

    // Both queries count listens in [lo, hi) like the charts in profiles.rs.
    //
    // The shared entries: counts of artists, releases and recordings for each
    // profile apart, keeping what both heard. Without a join of the two
    // profiles' counts (a generic plan of one took minutes), and in three
    // aggregates rather than grouping sets, which spilled to disk at 4 MB
    // work_mem with a long history. Names only for the entries shown. The
    // totals are materialized: a generic plan summed them again for every entry.
    let shared = sqlx::query!(
        r#"WITH span AS (
               SELECT coalesce($3::date::timestamp AT TIME ZONE $5, '-infinity') AS lo,
                      coalesce(($4::date + 1)::timestamp AT TIME ZONE $5, 'infinity') AS hi
           ), period AS (
               SELECT l.profile_id, l.artist_id, l.release_id, l.recording_id
                 FROM span, listen l
                WHERE l.profile_id IN ($1, $2)
                  AND l.listened_at >= span.lo AND l.listened_at < span.hi
                  AND ($7::text IS NULL OR listen_source(l.client) = $7)
           ), counted AS MATERIALIZED (
               SELECT 'artist' AS kind, artist_id AS id,
                      count(*) FILTER (WHERE profile_id = $1) AS a,
                      count(*) FILTER (WHERE profile_id = $2) AS b
                 FROM period
                GROUP BY 2
               UNION ALL
               SELECT 'release', release_id,
                      count(*) FILTER (WHERE profile_id = $1),
                      count(*) FILTER (WHERE profile_id = $2)
                 FROM period
                WHERE release_id IS NOT NULL
                GROUP BY 2
               UNION ALL
               SELECT 'recording', recording_id,
                      count(*) FILTER (WHERE profile_id = $1),
                      count(*) FILTER (WHERE profile_id = $2)
                 FROM period
                GROUP BY 2
           ), totals AS MATERIALIZED (
               SELECT sum(a)::float8 AS a, sum(b)::float8 AS b FROM counted WHERE kind = 'artist'
           ), ranked AS (
               SELECT c.kind, c.id, c.a, c.b,
                      row_number() OVER (PARTITION BY c.kind
                                         ORDER BY least(c.a / t.a, c.b / t.b) DESC, c.a + c.b DESC, c.id)
                          AS rank,
                      sum(least(c.a / t.a, c.b / t.b)) OVER (PARTITION BY c.kind) AS overlap,
                      count(*) OVER (PARTITION BY c.kind) AS shared
                 FROM counted c
                CROSS JOIN totals t
                WHERE c.a > 0 AND c.b > 0
           ), top AS MATERIALIZED (
               SELECT * FROM ranked WHERE rank <= $6
           )
           SELECT k.kind AS "kind!", k.id AS "id!", k.a AS "a!", k.b AS "b!",
                  k.overlap AS "overlap!", k.shared AS "shared!",
                  CASE k.kind
                      WHEN 'artist' THEN (SELECT name FROM artist WHERE id = k.id)
                      WHEN 'release' THEN (SELECT title FROM release WHERE id = k.id)
                      ELSE (SELECT title FROM recording WHERE id = k.id)
                  END AS "name!",
                  CASE k.kind
                      WHEN 'release' THEN (SELECT a.name FROM release r JOIN artist a ON a.id = r.artist_id
                                            WHERE r.id = k.id)
                      WHEN 'recording' THEN (SELECT a.name FROM recording r JOIN artist a ON a.id = r.artist_id
                                              WHERE r.id = k.id)
                  END AS "artist?"
             FROM top k
            ORDER BY k.kind, k.rank"#,
        a.id,
        b.id,
        from,
        to,
        tz,
        limit,
        source,
    )
    .fetch_all(&state.db);

    // Each profile's listens and artists in the period, and its most heard
    // artists that the other profile has no listen of at all, in any period.
    // One pass over both profiles' whole history counts both.
    let sides = sqlx::query!(
        r#"WITH span AS (
               SELECT coalesce($3::date::timestamp AT TIME ZONE $5, '-infinity') AS lo,
                      coalesce(($4::date + 1)::timestamp AT TIME ZONE $5, 'infinity') AS hi
           ), counted AS MATERIALIZED (
               SELECT l.artist_id AS id,
                      count(*) FILTER (WHERE l.profile_id = $1 AND l.listened_at >= span.lo
                                         AND l.listened_at < span.hi
                                         AND ($7::text IS NULL OR listen_source(l.client) = $7)) AS a,
                      count(*) FILTER (WHERE l.profile_id = $2 AND l.listened_at >= span.lo
                                         AND l.listened_at < span.hi
                                         AND ($7::text IS NULL OR listen_source(l.client) = $7)) AS b,
                      bool_or(l.profile_id = $1) AS heard_a,
                      bool_or(l.profile_id = $2) AS heard_b
                 FROM span, listen l
                WHERE l.profile_id IN ($1, $2)
                GROUP BY 1
           ), stats AS (
               SELECT coalesce(sum(a), 0)::bigint AS listens_a, count(*) FILTER (WHERE a > 0) AS artists_a,
                      count(*) FILTER (WHERE a > 0 AND NOT heard_b) AS only_a,
                      coalesce(sum(b), 0)::bigint AS listens_b, count(*) FILTER (WHERE b > 0) AS artists_b,
                      count(*) FILTER (WHERE b > 0 AND NOT heard_a) AS only_b
                 FROM counted
           ), only_one AS (
               (SELECT false AS second, id, a AS listens FROM counted
                 WHERE a > 0 AND NOT heard_b ORDER BY a DESC, id LIMIT $6)
               UNION ALL
               (SELECT true, id, b FROM counted
                 WHERE b > 0 AND NOT heard_a ORDER BY b DESC, id LIMIT $6)
           )
           SELECT s.listens_a AS "listens_a!", s.artists_a AS "artists_a!", s.only_a AS "only_a!",
                  s.listens_b AS "listens_b!", s.artists_b AS "artists_b!", s.only_b AS "only_b!",
                  o.second AS "second?", o.id AS "id?", ar.name AS "name?", o.listens AS "listens?"
             FROM stats s
             LEFT JOIN only_one o ON true
             LEFT JOIN artist ar ON ar.id = o.id
            ORDER BY o.second, o.listens DESC, o.id"#,
        a.id,
        b.id,
        from,
        to,
        tz,
        limit,
        source,
    )
    .fetch_all(&state.db);

    let (shared, sides) = tokio::try_join!(shared, sides)?;

    let mut score = 0.0;
    let mut counts = SharedCounts::default();
    let (mut artists, mut releases, mut recordings) = (Vec::new(), Vec::new(), Vec::new());
    for row in shared {
        let (list, count) = match row.kind.as_str() {
            "artist" => {
                score = row.overlap;
                (&mut artists, &mut counts.artists)
            }
            "release" => (&mut releases, &mut counts.releases),
            _ => (&mut recordings, &mut counts.recordings),
        };
        *count = row.shared;
        list.push(SharedEntry {
            id: row.id,
            name: row.name,
            artist: row.artist,
            a: row.a,
            b: row.b,
        });
    }

    let side = |profile: Profile, second: bool| {
        let stats = &sides[0];
        let (listens, artists, only_count) = if second {
            (stats.listens_b, stats.artists_b, stats.only_b)
        } else {
            (stats.listens_a, stats.artists_a, stats.only_a)
        };
        let only = sides
            .iter()
            .filter(|row| row.second == Some(second))
            .filter_map(|row| {
                Some(OnlyEntry {
                    id: row.id?,
                    name: row.name.clone()?,
                    listens: row.listens?,
                })
            })
            .collect();
        Side {
            username: profile.username,
            slug: profile.slug,
            name: profile.name,
            listens,
            artists,
            only_count,
            only,
        }
    };

    Ok(Json(Comparison {
        a: side(a, false),
        b: side(b, true),
        // Rounding in the sum can go a hair above 1.
        score: score.clamp(0.0, 1.0),
        shared: counts,
        artists,
        releases,
        recordings,
    }))
}
