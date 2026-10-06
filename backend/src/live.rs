//! Live updates for open profile pages: a profile page keeps one Server-Sent
//! Events stream open and reloads "now playing" or the recent listens when the
//! stream says they changed.
//!
//! Whoever changes them sends a PostgreSQL NOTIFY (see [`notify`]), so changes
//! from other processes reach the browser too, such as a `musicbanana import
//! yourspotify --every` next to the server. The server listens once and hands
//! each notification to the streams of that profile.

use std::{convert::Infallible, time::Duration};

use axum::{
    Router,
    extract::{Path, State},
    http::HeaderValue,
    response::{
        IntoResponse, Response,
        sse::{Event, KeepAlive, Sse},
    },
    routing::get,
};
use futures_util::stream::{self, Stream};
use sqlx::{PgPool, postgres::PgListener};
use tokio::sync::{broadcast, watch};

use crate::{AppError, AppState, account::Viewer, profiles::find_profile};

/// The NOTIFY channel; the payload is `<profile id> <what>`.
const CHANNEL: &str = "musicbanana_live";

/// What changed for a profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    NowPlaying,
    Listens,
}

impl Change {
    fn name(self) -> &'static str {
        match self {
            Change::NowPlaying => "now-playing",
            Change::Listens => "listens",
        }
    }

    fn parse(name: &str) -> Option<Self> {
        match name {
            "now-playing" => Some(Change::NowPlaying),
            "listens" => Some(Change::Listens),
            _ => None,
        }
    }
}

/// Tells the open pages of a profile that something changed. Inside a
/// transaction, PostgreSQL sends it on commit.
pub async fn notify<'e>(
    db: impl sqlx::PgExecutor<'e>,
    profile_id: i64,
    change: Change,
) -> sqlx::Result<()> {
    sqlx::query("SELECT pg_notify($1, $2)")
        .bind(CHANNEL)
        .bind(format!("{profile_id} {}", change.name()))
        .execute(db)
        .await?;
    Ok(())
}

/// Hands the notifications to the open streams, and ends the streams when the
/// server shuts down.
#[derive(Clone)]
pub struct Hub {
    changes: broadcast::Sender<(i64, Change)>,
    closed: watch::Sender<bool>,
}

impl Default for Hub {
    fn default() -> Self {
        Self {
            changes: broadcast::channel(256).0,
            closed: watch::channel(false).0,
        }
    }
}

impl Hub {
    /// Ends every stream, so a graceful shutdown need not wait for open pages.
    pub fn close(&self) {
        self.closed.send_replace(true);
    }

    /// Listens for the notifications of all processes as long as the server
    /// runs. The listener reconnects by itself after the database went away.
    pub fn spawn_listener(&self, db: PgPool) {
        let changes = self.changes.clone();
        tokio::spawn(async move {
            loop {
                if let Err(error) = listen(&db, &changes).await {
                    tracing::warn!("live updates: {error}");
                }
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        });
    }
}

async fn listen(db: &PgPool, changes: &broadcast::Sender<(i64, Change)>) -> sqlx::Result<()> {
    let mut listener = PgListener::connect_with(db).await?;
    listener.listen(CHANNEL).await?;
    loop {
        let notification = listener.recv().await?;
        let parsed = notification
            .payload()
            .split_once(' ')
            .and_then(|(id, change)| Some((id.parse::<i64>().ok()?, Change::parse(change)?)));
        if let Some(change) = parsed {
            // Nobody listening is fine.
            let _ = changes.send(change);
        }
    }
}

pub fn routes() -> Router<AppState> {
    Router::new().route("/profiles/{username}/{slug}/live", get(live))
}

/// The changes of one profile as Server-Sent Events, named and with the name as
/// data: `now-playing` when that changed, `listens` when new listens came in (the page then reloads
/// "now playing" as well). After a reconnect the page reloads both by itself,
/// since it may have missed events in between.
async fn live(
    State(state): State<AppState>,
    viewer: Viewer,
    Path((username, slug)): Path<(String, String)>,
) -> Result<Response, AppError> {
    let profile = find_profile(&state.db, viewer, &username, &slug).await?;
    let events = changes(&state.live, profile.id);
    let mut response = Sse::new(events)
        .keep_alive(KeepAlive::default())
        .into_response();
    // nginx would otherwise hold the events back in its buffer.
    response
        .headers_mut()
        .insert("x-accel-buffering", HeaderValue::from_static("no"));
    Ok(response)
}

fn changes(hub: &Hub, profile_id: i64) -> impl Stream<Item = Result<Event, Infallible>> + use<> {
    let state = (hub.changes.subscribe(), hub.closed.subscribe());
    stream::unfold(state, move |(mut rx, mut closed)| async move {
        loop {
            if *closed.borrow() {
                return None;
            }
            let change = tokio::select! {
                received = rx.recv() => match received {
                    Ok((id, change)) if id == profile_id => change,
                    Ok(_) => continue,
                    // Too slow to keep up: the page reloads everything on `listens`.
                    Err(broadcast::error::RecvError::Lagged(_)) => Change::Listens,
                    Err(broadcast::error::RecvError::Closed) => return None,
                },
                _ = closed.changed() => return None,
            };
            // Browsers drop an event without data.
            let event = Event::default().event(change.name()).data(change.name());
            return Some((Ok(event), (rx, closed)));
        }
    })
}
