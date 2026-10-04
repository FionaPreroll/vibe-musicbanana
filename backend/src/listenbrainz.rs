//! The part of the ListenBrainz API that scrobbling clients use, mounted at
//! `/api/listenbrainz/1/`. A player or server with ListenBrainz support, such as
//! Navidrome, scrobbles to musicbanana once its ListenBrainz base URL points
//! there and it has a musicbanana token (see [`crate::tokens`]).
//!
//! Requests are checked like listenbrainz-server does (same limits, same error
//! format `{"code": …, "error": …}`). A few things it rejects are accepted here
//! because they do no harm: extra keys, `listened_at` on `playing_now`, and
//! durations or track numbers that are not usable (they are left out).
//! Navidrome retries a listen after a 5xx and drops it after any other error, so
//! a 4xx must only mean that the listen can never be accepted.

use axum::{
    Json, Router,
    body::Bytes,
    extract::{DefaultBodyLimit, Query, State, rejection::BytesRejection},
    http::{HeaderMap, StatusCode, header::AUTHORIZATION},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{
    AppState,
    catalog::{Mbids, name_key},
    scrobble, tokens,
};

// Limits as in listenbrainz-server (listenbrainz/webserver/views/api_tools.py).
const MAX_LISTEN_SIZE: usize = 10_240;
const MAX_LISTENS_PER_REQUEST: usize = 1_000;
const MAX_PAYLOAD_SIZE: usize = MAX_LISTEN_SIZE * MAX_LISTENS_PER_REQUEST;
/// 2002-10-01 00:00:00 UTC; nothing was scrobbled before that.
const MIN_LISTENED_AT: i64 = 1_033_430_400;
/// Clocks run fast sometimes: up to an hour in the future is fine.
const ALLOWED_SKEW_SECONDS: i64 = 60 * 60;
/// 24 days.
const MAX_DURATION_MS: f64 = 24.0 * 24.0 * 60.0 * 60.0 * 1000.0;
/// Names are looked up through B-tree indexes, whose entries cannot be much
/// larger than 2.7 kB.
const MAX_NAME_KEY_BYTES: usize = 2_000;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/submit-listens",
            post(submit_listens).layer(DefaultBodyLimit::max(MAX_PAYLOAD_SIZE)),
        )
        .route("/validate-token", get(validate_token))
}

/// An error response as ListenBrainz sends it.
#[derive(Debug)]
struct LbError {
    status: StatusCode,
    message: String,
}

impl LbError {
    fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }

    fn bad_request(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, message)
    }

    fn unauthorized(message: &str) -> Self {
        Self::new(StatusCode::UNAUTHORIZED, message)
    }
}

impl IntoResponse for LbError {
    fn into_response(self) -> Response {
        let body = json!({ "code": self.status.as_u16(), "error": self.message });
        (self.status, Json(body)).into_response()
    }
}

impl From<sqlx::Error> for LbError {
    fn from(err: sqlx::Error) -> Self {
        tracing::error!("{err:#}");
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Something went wrong, please try again later.",
        )
    }
}

/// The token from `Authorization: Token <token>`, or `Bearer <token>`.
fn header_token(headers: &HeaderMap) -> Option<&str> {
    let (scheme, token) = headers
        .get(AUTHORIZATION)?
        .to_str()
        .ok()?
        .trim()
        .split_once(' ')?;
    let token = token.trim();
    let known = scheme.eq_ignore_ascii_case("token") || scheme.eq_ignore_ascii_case("bearer");
    (known && !token.is_empty()).then_some(token)
}

#[derive(Deserialize)]
struct ValidateParams {
    /// Older clients send the token as `?token=` instead of the header.
    token: Option<String>,
}

async fn validate_token(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<ValidateParams>,
) -> Result<Json<Value>, LbError> {
    let token = header_token(&headers)
        .map(str::to_owned)
        .or(params.token)
        .filter(|token| !token.trim().is_empty())
        .ok_or_else(|| LbError::bad_request("You need to provide an Authorization token."))?;
    // An unknown token is not an error here, just "valid": false.
    let body = match tokens::authenticate(&state.db, &token).await? {
        Some(owner) => json!({
            "code": 200,
            "message": "Token valid.",
            "valid": true,
            "user_name": owner.username,
        }),
        None => json!({ "code": 200, "message": "Token invalid.", "valid": false }),
    };
    Ok(Json(body))
}

async fn submit_listens(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Result<Bytes, BytesRejection>,
) -> Result<Json<Value>, LbError> {
    if !headers.contains_key(AUTHORIZATION) {
        return Err(LbError::unauthorized(
            "You need to provide an Authorization header.",
        ));
    }
    let token = header_token(&headers)
        .ok_or_else(|| LbError::unauthorized("Provided Authorization header is invalid."))?;
    let owner = tokens::authenticate(&state.db, token)
        .await?
        .ok_or_else(|| LbError::unauthorized("Invalid authorization token."))?;

    let body = body.map_err(|rejection| match rejection.status() {
        StatusCode::PAYLOAD_TOO_LARGE => LbError::bad_request(format!(
            "Payload too large. Payload cannot exceed {MAX_PAYLOAD_SIZE} bytes"
        )),
        status => LbError::new(status, rejection.body_text()),
    })?;

    match parse_submission(&body, OffsetDateTime::now_utc())? {
        Submission::Listens(listens) => {
            scrobble::record(&state.db, owner.profile_id, &listens).await?;
        }
        Submission::PlayingNow(playing) => {
            scrobble::set_now_playing(&state.db, owner.profile_id, &playing).await?;
        }
    }
    Ok(Json(json!({ "status": "ok" })))
}

#[derive(Debug)]
enum Submission {
    Listens(Vec<scrobble::Listen>),
    PlayingNow(scrobble::NowPlaying),
}

#[derive(Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum ListenType {
    Single,
    Import,
    PlayingNow,
}

#[derive(Deserialize)]
struct RawSubmission {
    listen_type: ListenType,
    payload: Vec<RawListen>,
}

#[derive(Deserialize)]
struct RawListen {
    #[serde(default)]
    listened_at: Option<Value>,
    track_metadata: TrackMetadata,
}

#[derive(Deserialize)]
struct TrackMetadata {
    artist_name: String,
    track_name: String,
    #[serde(default)]
    release_name: Option<String>,
    #[serde(default)]
    additional_info: Option<Map<String, Value>>,
}

/// Checks a request body the way listenbrainz-server does.
fn parse_submission(body: &[u8], now: OffsetDateTime) -> Result<Submission, LbError> {
    let raw: RawSubmission = serde_json::from_slice(body)
        .map_err(|err| LbError::bad_request(format!("Cannot parse JSON document: {err}")))?;
    let count = raw.payload.len();
    if count == 0 {
        return Err(LbError::bad_request(
            "JSON document does not contain any listens",
        ));
    }
    if count > MAX_LISTENS_PER_REQUEST {
        return Err(LbError::bad_request(format!(
            "Too many listens. You may not submit more than {MAX_LISTENS_PER_REQUEST} listens at once."
        )));
    }
    if body.len() > count * MAX_LISTEN_SIZE {
        return Err(LbError::bad_request(format!(
            "JSON document is too large. Each listen may not be larger than {MAX_LISTEN_SIZE} bytes."
        )));
    }
    if raw.listen_type != ListenType::Import && count > 1 {
        return Err(LbError::bad_request(
            "JSON document contains more than one listen for a single/playing_now. It should contain only one.",
        ));
    }

    if raw.listen_type == ListenType::PlayingNow {
        let listen = raw.payload.into_iter().next().expect("exactly one listen");
        let track = check_track(listen.track_metadata)?;
        return Ok(Submission::PlayingNow(scrobble::NowPlaying {
            artist: track.artist,
            track: track.track,
            album: track.album,
            duration_ms: track.duration_ms,
        }));
    }

    let listens = raw
        .payload
        .into_iter()
        .map(|listen| {
            let listened_at = listened_at(listen.listened_at.as_ref(), now)?;
            let track = check_track(listen.track_metadata)?;
            Ok(scrobble::Listen {
                listened_at,
                artist: track.artist,
                track: track.track,
                album: track.album,
                track_number: track.track_number,
                duration_ms: track.duration_ms,
                client: track.client,
                mbids: track.mbids,
                extra: track.extra,
            })
        })
        .collect::<Result<_, LbError>>()?;
    Ok(Submission::Listens(listens))
}

/// `listened_at` read like ListenBrainz does (Python's `int()`): an integer, a
/// number whose fraction is dropped, or a string of digits.
fn listened_at(value: Option<&Value>, now: OffsetDateTime) -> Result<OffsetDateTime, LbError> {
    let value = value.ok_or_else(|| {
        LbError::bad_request("JSON document must contain the key listened_at at the top level.")
    })?;
    let seconds = match value {
        Value::Number(n) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
    .ok_or_else(|| {
        LbError::bad_request("JSON document must contain an int value for listened_at.")
    })?;
    if seconds >= now.unix_timestamp() + ALLOWED_SKEW_SECONDS {
        return Err(LbError::bad_request(
            "Value for key listened_at is too high.",
        ));
    }
    if seconds < MIN_LISTENED_AT {
        return Err(LbError::bad_request(format!(
            "Value for key listened_at is too low. listened_at timestamp should be greater than {MIN_LISTENED_AT} (2002-10-01 00:00:00 UTC)."
        )));
    }
    OffsetDateTime::from_unix_timestamp(seconds)
        .map_err(|err| LbError::bad_request(format!("Invalid listened_at: {err}")))
}

/// The checked contents of `track_metadata`.
struct Track {
    artist: String,
    track: String,
    album: Option<String>,
    track_number: Option<i16>,
    duration_ms: Option<i32>,
    client: Option<String>,
    mbids: Mbids,
    extra: Option<Value>,
}

fn check_track(meta: TrackMetadata) -> Result<Track, LbError> {
    let artist = required_name(meta.artist_name, "artist_name")?;
    let track = required_name(meta.track_name, "track_name")?;
    // An empty release_name means there is none.
    let album = meta.release_name.filter(|name| !name.trim().is_empty());
    if let Some(album) = &album {
        check_name(album, "release_name")?;
    }
    let info = meta.additional_info.unwrap_or_default();
    // PostgreSQL text and jsonb cannot hold NUL.
    if map_contains_nul(&info) {
        return Err(LbError::bad_request(
            "track_metadata.additional_info contains a unicode null",
        ));
    }
    Ok(Track {
        artist,
        track,
        album,
        track_number: info.get("tracknumber").and_then(track_number),
        duration_ms: duration_ms(&info),
        client: client(&info),
        mbids: mbids(&info),
        extra: (!info.is_empty()).then_some(Value::Object(info)),
    })
}

fn required_name(name: String, key: &str) -> Result<String, LbError> {
    if name.trim().is_empty() {
        return Err(LbError::bad_request(format!(
            "field track_metadata.{key} is empty."
        )));
    }
    check_name(&name, key)?;
    Ok(name)
}

fn check_name(name: &str, key: &str) -> Result<(), LbError> {
    if name.contains('\0') {
        return Err(LbError::bad_request(format!(
            "track_metadata.{key} contains a unicode null"
        )));
    }
    if name_key(name).len() > MAX_NAME_KEY_BYTES {
        return Err(LbError::bad_request(format!(
            "track_metadata.{key} is too long."
        )));
    }
    Ok(())
}

fn map_contains_nul(map: &Map<String, Value>) -> bool {
    map.iter()
        .any(|(key, value)| key.contains('\0') || contains_nul(value))
}

fn contains_nul(value: &Value) -> bool {
    match value {
        Value::String(s) => s.contains('\0'),
        Value::Array(items) => items.iter().any(contains_nul),
        Value::Object(map) => map_contains_nul(map),
        _ => false,
    }
}

/// `tracknumber`: a number, or a string such as "3" or "3/12".
fn track_number(value: &Value) -> Option<i16> {
    let n = match value {
        Value::Number(n) => n.as_i64()?,
        Value::String(s) => {
            let digits: String = s.trim().chars().take_while(char::is_ascii_digit).collect();
            digits.parse().ok()?
        }
        _ => return None,
    };
    i16::try_from(n).ok().filter(|&n| n >= 0)
}

/// The track length from `duration_ms`, or from `duration` in seconds.
fn duration_ms(info: &Map<String, Value>) -> Option<i32> {
    let ms = match (info.get("duration_ms"), info.get("duration")) {
        (Some(ms), _) => number(ms)?,
        (None, Some(seconds)) => number(seconds)? * 1000.0,
        (None, None) => return None,
    };
    (1.0..=MAX_DURATION_MS).contains(&ms).then_some(ms as i32)
}

fn number(value: &Value) -> Option<f64> {
    match value {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

/// The MusicBrainz IDs of the artist, release group and recording. An artist ID
/// counts only for a track by one artist: "A feat. B" with the IDs of A and B is
/// no artist of its own, and Navidrome sends `[""]` for an artist without an ID.
fn mbids(info: &Map<String, Value>) -> Mbids {
    let id = |value: &Value| {
        value
            .as_str()
            .and_then(|s| Uuid::try_parse(s.trim()).ok())
            .filter(|id| !id.is_nil())
    };
    let list = |key: &str| info.get(key).and_then(Value::as_array);
    let artist = match (
        list("artist_mbids").map(Vec::as_slice),
        list("artist_names").map(Vec::len),
    ) {
        (Some([single]), None | Some(1)) => id(single),
        _ => None,
    };
    Mbids {
        artist,
        release_group: info.get("release_group_mbid").and_then(id),
        recording: info.get("recording_mbid").and_then(id),
    }
}

/// "Navidrome 0.58.0" from `submission_client` and its version, or else the
/// media player and its version.
fn client(info: &Map<String, Value>) -> Option<String> {
    let text = |key: &str| {
        info.get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
    };
    let (name, version) = text("submission_client")
        .map(|name| (name, text("submission_client_version")))
        .or_else(|| text("media_player").map(|name| (name, text("media_player_version"))))?;
    Some(match version {
        Some(version) => format!("{name} {version}"),
        None => name.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_790_000_000; // 2026-09-21

    fn parse(body: Value) -> Result<Submission, LbError> {
        parse_submission(
            body.to_string().as_bytes(),
            OffsetDateTime::from_unix_timestamp(NOW).unwrap(),
        )
    }

    fn listens(body: Value) -> Vec<scrobble::Listen> {
        match parse(body) {
            Ok(Submission::Listens(listens)) => listens,
            other => panic!("expected listens, got {other:?}"),
        }
    }

    fn error(body: Value) -> String {
        match parse(body) {
            Err(err) => {
                assert_eq!(err.status, StatusCode::BAD_REQUEST);
                err.message
            }
            Ok(other) => panic!("expected an error, got {other:?}"),
        }
    }

    fn single(listened_at: Value, track_metadata: Value) -> Value {
        json!({
            "listen_type": "single",
            "payload": [{ "listened_at": listened_at, "track_metadata": track_metadata }],
        })
    }

    fn unrockbar() -> Value {
        json!({ "artist_name": "Die Ärzte", "track_name": "Unrockbar" })
    }

    #[test]
    fn navidrome_listen() {
        let listen = listens(single(
            json!(1_789_999_000),
            json!({
                "artist_name": "Die Ärzte",
                "track_name": "Unrockbar",
                "release_name": "Geräusch",
                "additional_info": {
                    "submission_client": "Navidrome",
                    "submission_client_version": "0.58.0 (1a2b3c4)",
                    "tracknumber": 3,
                    "artist_names": ["Die Ärzte"],
                    "artist_mbids": ["4d0b4a68-4d8a-4c5e-9a8f-1f0e4f3b2a10"],
                    "recording_mbid": "6a0c5c43-7d0e-4f5c-a0a4-49d3e1d6b6b2",
                    "release_mbid": "0c7f2e49-58a1-4bd5-8a7f-3f5b6c0d9e21",
                    "release_group_mbid": "9b3e5d70-2c1f-4f6a-b8d4-7e2a1c0f5b32",
                    "duration_ms": 196_000,
                },
            }),
        ))
        .remove(0);
        assert_eq!(listen.listened_at.unix_timestamp(), 1_789_999_000);
        assert_eq!(listen.artist, "Die Ärzte");
        assert_eq!(listen.track, "Unrockbar");
        assert_eq!(listen.album.as_deref(), Some("Geräusch"));
        assert_eq!(listen.track_number, Some(3));
        assert_eq!(listen.duration_ms, Some(196_000));
        assert_eq!(listen.client.as_deref(), Some("Navidrome 0.58.0 (1a2b3c4)"));
        let id = |s| Some(Uuid::parse_str(s).unwrap());
        assert_eq!(
            listen.mbids,
            Mbids {
                artist: id("4d0b4a68-4d8a-4c5e-9a8f-1f0e4f3b2a10"),
                release_group: id("9b3e5d70-2c1f-4f6a-b8d4-7e2a1c0f5b32"),
                recording: id("6a0c5c43-7d0e-4f5c-a0a4-49d3e1d6b6b2"),
            }
        );
        let extra = listen.extra.unwrap();
        assert_eq!(
            extra["recording_mbid"],
            "6a0c5c43-7d0e-4f5c-a0a4-49d3e1d6b6b2"
        );
        assert_eq!(extra["artist_names"], json!(["Die Ärzte"]));
    }

    #[test]
    fn minimal_listen_has_no_extras() {
        let listen = listens(single(json!(1_789_999_000), unrockbar())).remove(0);
        assert_eq!(listen.album, None);
        assert_eq!(listen.track_number, None);
        assert_eq!(listen.duration_ms, None);
        assert_eq!(listen.client, None);
        assert_eq!(listen.mbids, Mbids::default());
        assert!(listen.extra.is_none());
    }

    #[test]
    fn an_artist_id_only_for_a_track_by_one_artist() {
        const A: &str = "4d0b4a68-4d8a-4c5e-9a8f-1f0e4f3b2a10";
        const B: &str = "5e1c5b79-5e9b-4d6f-8b90-2a1f5a4c3b21";
        let artist = |info: Value| {
            let meta = json!({ "artist_name": "Die Ärzte", "track_name": "Unrockbar", "additional_info": info });
            listens(single(json!(1_789_999_000), meta))
                .remove(0)
                .mbids
                .artist
                .map(|id| id.to_string())
        };
        assert_eq!(artist(json!({ "artist_mbids": [A] })).as_deref(), Some(A));
        assert_eq!(
            artist(json!({ "artist_mbids": [A], "artist_names": ["Die Ärzte"] })).as_deref(),
            Some(A)
        );
        assert_eq!(
            artist(json!({ "artist_mbids": [format!(" {}", A.to_uppercase())] })).as_deref(),
            Some(A)
        );
        for info in [
            // "Farin Urlaub feat. Bela B." is no artist of its own.
            json!({ "artist_mbids": [A, B] }),
            json!({ "artist_mbids": [A], "artist_names": ["Farin Urlaub", "Bela B."] }),
            // Navidrome for files without IDs.
            json!({ "artist_mbids": [""], "artist_names": ["Die Ärzte"] }),
            json!({ "artist_mbids": ["not an id"] }),
            json!({ "artist_mbids": ["00000000-0000-0000-0000-000000000000"] }),
            json!({ "artist_mbids": A }),
            json!({ "artist_mbids": [] }),
        ] {
            assert_eq!(artist(info.clone()), None, "{info}");
        }
    }

    #[test]
    fn listened_at_is_read_like_python_int() {
        for value in [
            json!(1_789_999_000),
            json!(1_789_999_000.9),
            json!(" 1789999000 "),
        ] {
            let listen = listens(single(value.clone(), unrockbar())).remove(0);
            assert_eq!(
                listen.listened_at.unix_timestamp(),
                1_789_999_000,
                "{value}"
            );
        }
        assert!(error(single(json!("yesterday"), unrockbar())).contains("int value"));
        assert!(error(single(json!(true), unrockbar())).contains("int value"));
    }

    #[test]
    fn listened_at_must_be_plausible() {
        let missing =
            json!({ "listen_type": "single", "payload": [{ "track_metadata": unrockbar() }] });
        assert!(error(missing).contains("must contain the key listened_at"));
        assert!(error(single(json!(0), unrockbar())).contains("too low"));
        assert!(error(single(json!(MIN_LISTENED_AT - 1), unrockbar())).contains("too low"));
        assert!(error(single(json!(NOW + 3600), unrockbar())).contains("too high"));
        // Clock skew of less than an hour is fine.
        listens(single(json!(MIN_LISTENED_AT), unrockbar()));
        listens(single(json!(NOW + 3599), unrockbar()));
    }

    #[test]
    fn names_are_required_and_kept_as_sent() {
        let listen = listens(single(
            json!(NOW),
            json!({ "artist_name": " Die Ärzte ", "track_name": "Unrockbar", "release_name": "  " }),
        ))
        .remove(0);
        assert_eq!(listen.artist, " Die Ärzte ");
        assert_eq!(listen.album, None, "a blank release_name means no album");

        let empty = json!({ "artist_name": "  ", "track_name": "Unrockbar" });
        assert_eq!(
            error(single(json!(NOW), empty)),
            "field track_metadata.artist_name is empty."
        );
        let missing = json!({ "artist_name": "Die Ärzte" });
        assert!(error(single(json!(NOW), missing)).contains("missing field `track_name`"));
        let not_a_string = json!({ "artist_name": "Die Ärzte", "track_name": 7 });
        assert!(error(single(json!(NOW), not_a_string)).contains("invalid type"));
        let too_long = json!({ "artist_name": "a".repeat(2001), "track_name": "Unrockbar" });
        assert!(error(single(json!(NOW), too_long)).contains("too long"));
    }

    #[test]
    fn nul_characters_are_rejected() {
        let in_name = json!({ "artist_name": "Die Ärzte\u{0}", "track_name": "Unrockbar" });
        assert!(error(single(json!(NOW), in_name)).contains("unicode null"));
        let in_info = json!({
            "artist_name": "Die Ärzte",
            "track_name": "Unrockbar",
            "additional_info": { "tags": ["punk", "\u{0}"] },
        });
        assert!(error(single(json!(NOW), in_info)).contains("unicode null"));
    }

    #[test]
    fn durations_and_track_numbers_are_read_leniently() {
        let info = |info: Value| {
            let meta = json!({ "artist_name": "Die Ärzte", "track_name": "Unrockbar", "additional_info": info });
            listens(single(json!(NOW), meta)).remove(0)
        };
        assert_eq!(info(json!({ "duration": 196 })).duration_ms, Some(196_000));
        assert_eq!(
            info(json!({ "duration_ms": "196000" })).duration_ms,
            Some(196_000)
        );
        assert_eq!(info(json!({ "duration_ms": 0 })).duration_ms, None);
        assert_eq!(info(json!({ "duration_ms": -5 })).duration_ms, None);
        assert_eq!(info(json!({ "duration": 1e12 })).duration_ms, None);
        assert_eq!(info(json!({ "duration": "long" })).duration_ms, None);
        assert_eq!(info(json!({ "tracknumber": "3/12" })).track_number, Some(3));
        assert_eq!(info(json!({ "tracknumber": "B2" })).track_number, None);
        assert_eq!(info(json!({ "tracknumber": 70_000 })).track_number, None);
        assert_eq!(
            info(json!({ "media_player": "Supersonic", "media_player_version": "0.15" }))
                .client
                .as_deref(),
            Some("Supersonic 0.15")
        );
    }

    #[test]
    fn listen_types_and_counts() {
        let listen = |ts: i64| json!({ "listened_at": ts, "track_metadata": unrockbar() });
        let import =
            json!({ "listen_type": "import", "payload": [listen(NOW - 2), listen(NOW - 1)] });
        assert_eq!(listens(import).len(), 2);

        let two_singles =
            json!({ "listen_type": "single", "payload": [listen(NOW - 2), listen(NOW - 1)] });
        assert!(error(two_singles).contains("only one"));
        let empty = json!({ "listen_type": "import", "payload": [] });
        assert!(error(empty).contains("does not contain any listens"));
        let too_many = json!({ "listen_type": "import", "payload": vec![listen(NOW); 1001] });
        assert!(error(too_many).contains("Too many listens"));
        let unknown = json!({ "listen_type": "radio", "payload": [listen(NOW)] });
        assert!(error(unknown).contains("unknown variant `radio`"));
        assert!(
            parse_submission(b"{not json", OffsetDateTime::now_utc())
                .unwrap_err()
                .message
                .starts_with("Cannot parse JSON document")
        );
    }

    #[test]
    fn playing_now_needs_no_listened_at() {
        let body = json!({
            "listen_type": "playing_now",
            "payload": [{ "track_metadata": {
                "artist_name": "Die Ärzte",
                "track_name": "Unrockbar",
                "release_name": "Geräusch",
                "additional_info": { "duration_ms": 196_000 },
            }}],
        });
        match parse(body) {
            Ok(Submission::PlayingNow(playing)) => {
                assert_eq!(playing.track, "Unrockbar");
                assert_eq!(playing.album.as_deref(), Some("Geräusch"));
                assert_eq!(playing.duration_ms, Some(196_000));
            }
            other => panic!("expected playing_now, got {other:?}"),
        }
    }

    #[test]
    fn authorization_header() {
        let headers = |value: &str| {
            let mut headers = HeaderMap::new();
            headers.insert(AUTHORIZATION, value.parse().unwrap());
            headers
        };
        assert_eq!(header_token(&headers("Token abc")), Some("abc"));
        assert_eq!(header_token(&headers("token  abc ")), Some("abc"));
        assert_eq!(header_token(&headers("Bearer abc")), Some("abc"));
        assert_eq!(header_token(&headers("Basic abc")), None);
        assert_eq!(header_token(&headers("abc")), None);
        assert_eq!(header_token(&HeaderMap::new()), None);
    }
}
