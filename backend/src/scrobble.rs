//! Storing what clients scrobble, independent of the protocol they speak.

use serde_json::Value;
use sqlx::PgPool;
use time::OffsetDateTime;

use crate::catalog::{Mbids, Resolver};

/// One listen as a client sent it. The strings are stored unchanged as the raw
/// data of the listen; the catalog gets the trimmed names.
#[derive(Debug)]
pub struct Listen {
    pub listened_at: OffsetDateTime,
    pub artist: String,
    pub track: String,
    pub album: Option<String>,
    pub track_number: Option<i16>,
    pub duration_ms: Option<i32>,
    /// Name and version of the sending program, e.g. "Navidrome 0.58.0".
    pub client: Option<String>,
    /// The MusicBrainz IDs among `extra` that tell the catalog entries apart.
    pub mbids: Mbids,
    /// Anything else the client sent along (MusicBrainz IDs, tags, …).
    pub extra: Option<Value>,
}

/// Stores listens and returns how many were new.
///
/// A listen is skipped as a duplicate when the profile already has one at the
/// same time (a resubmission), or one of the same recording closer than a player
/// needs to count a play: half the track, at most four minutes, or 15 seconds
/// when the length is unknown. Nobody plays a track twice in less time than that,
/// so such a pair comes from two scrobblers or a client that sent a play twice.
/// The same holds between listens of one batch.
pub async fn record(db: &PgPool, profile_id: i64, listens: &[Listen]) -> sqlx::Result<u64> {
    let mut catalog = Resolver::new(db);
    let mut artist_ids = Vec::with_capacity(listens.len());
    let mut recording_ids = Vec::with_capacity(listens.len());
    let mut release_ids = Vec::with_capacity(listens.len());
    for listen in listens {
        let mbids = listen.mbids;
        let artist_id = catalog.artist(&listen.artist, mbids.artist).await?;
        recording_ids.push(
            catalog
                .recording(
                    artist_id,
                    &listen.track,
                    listen.duration_ms,
                    mbids.recording,
                )
                .await?,
        );
        release_ids.push(match &listen.album {
            Some(album) => Some(
                catalog
                    .release(artist_id, album, mbids.release_group)
                    .await?,
            ),
            None => None,
        });
        artist_ids.push(artist_id);
    }

    // One array per column; UNNEST turns them back into rows.
    let listened_at: Vec<_> = listens.iter().map(|l| l.listened_at).collect();
    let artist: Vec<_> = listens.iter().map(|l| l.artist.as_str()).collect();
    let track: Vec<_> = listens.iter().map(|l| l.track.as_str()).collect();
    let album: Vec<_> = listens.iter().map(|l| l.album.as_deref()).collect();
    let track_number: Vec<_> = listens.iter().map(|l| l.track_number).collect();
    let duration_ms: Vec<_> = listens.iter().map(|l| l.duration_ms).collect();
    let client: Vec<_> = listens.iter().map(|l| l.client.as_deref()).collect();
    let extra: Vec<_> = listens
        .iter()
        .map(|l| l.extra.as_ref().map(Value::to_string))
        .collect();
    let inserted = sqlx::query!(
        "WITH t AS (
             SELECT t.*,
                    make_interval(secs => LEAST(COALESCE(t.duration_ms, r.length_ms, 30000) / 2000.0,
                                                240)::float8) AS gap,
                    lag(t.listened_at) OVER (PARTITION BY t.recording_id
                                             ORDER BY t.listened_at) AS previous
               FROM UNNEST($2::timestamptz[], $3::text[], $4::text[], $5::text[], $6::int2[],
                           $7::int4[], $8::text[], $9::text[], $10::int8[], $11::int8[], $12::int8[])
                 AS t(listened_at, artist_raw, track_raw, album_raw, track_number,
                      duration_ms, client, extra, artist_id, recording_id, release_id)
               JOIN recording r ON r.id = t.recording_id)
         INSERT INTO listen (profile_id, listened_at, artist_raw, track_raw, album_raw,
                             track_number, duration_ms, client, extra,
                             artist_id, recording_id, release_id)
         SELECT $1, t.listened_at, t.artist_raw, t.track_raw, t.album_raw,
                t.track_number, t.duration_ms, t.client, t.extra::jsonb,
                t.artist_id, t.recording_id, t.release_id
           FROM t
          WHERE (t.previous IS NULL OR t.listened_at - t.previous >= t.gap)
            AND NOT EXISTS (SELECT FROM listen o
                             WHERE o.profile_id = $1
                               AND o.recording_id = t.recording_id
                               AND o.listened_at > t.listened_at - t.gap
                               AND o.listened_at < t.listened_at + t.gap)
         ON CONFLICT (profile_id, listened_at) DO NOTHING",
        profile_id,
        &listened_at,
        &artist as &[&str],
        &track as &[&str],
        &album as &[Option<&str>],
        &track_number as &[Option<i16>],
        &duration_ms as &[Option<i32>],
        &client as &[Option<&str>],
        &extra as &[Option<String>],
        &artist_ids,
        &recording_ids,
        &release_ids as &[Option<i64>],
    )
    .execute(db)
    .await?;
    Ok(inserted.rows_affected())
}

/// What a client reports when a track starts.
#[derive(Debug)]
pub struct NowPlaying {
    pub artist: String,
    pub track: String,
    pub album: Option<String>,
    pub duration_ms: Option<i32>,
}

/// How long "now playing" shows when the client does not send the track length.
const NOW_PLAYING_FALLBACK_MS: i32 = 10 * 60 * 1000;

/// Replaces the profile's "now playing". It ends by itself after the track's
/// length, or after ten minutes when that is unknown (as on ListenBrainz).
pub async fn set_now_playing(
    db: &PgPool,
    profile_id: i64,
    playing: &NowPlaying,
) -> sqlx::Result<()> {
    sqlx::query!(
        "INSERT INTO now_playing (profile_id, started_at, expires_at,
                                  artist_raw, track_raw, album_raw, duration_ms)
         VALUES ($1, now(), now() + make_interval(secs => $6::int4 / 1000.0), $2, $3, $4, $5)
         ON CONFLICT (profile_id) DO UPDATE
            SET started_at = excluded.started_at,
                expires_at = excluded.expires_at,
                artist_raw = excluded.artist_raw,
                track_raw = excluded.track_raw,
                album_raw = excluded.album_raw,
                duration_ms = excluded.duration_ms",
        profile_id,
        playing.artist,
        playing.track,
        playing.album,
        playing.duration_ms,
        playing.duration_ms.unwrap_or(NOW_PLAYING_FALLBACK_MS),
    )
    .execute(db)
    .await?;
    Ok(())
}
