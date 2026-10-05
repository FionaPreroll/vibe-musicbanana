//! The status page for admins: build, process, database, the YourSpotify
//! connections and how long the API takes, per route since the server started.

use std::{
    collections::{HashMap, VecDeque},
    sync::{LazyLock, Mutex},
    time::{Duration, Instant},
};

use axum::{
    Json, Router,
    extract::{MatchedPath, Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
    routing::get,
};
use serde::Serialize;
use time::OffsetDateTime;

use crate::{
    AppError, AppState,
    account::Account,
    connections::{self, Connection},
};

pub fn routes() -> Router<AppState> {
    Router::new().route("/admin/status", get(status))
}

/// When the server (this process) started.
static STARTED: LazyLock<(Instant, OffsetDateTime)> =
    LazyLock::new(|| (Instant::now(), OffsetDateTime::now_utc()));

/// Called at start-up, so that the uptime counts from there.
pub fn started() {
    LazyLock::force(&STARTED);
}

// ---------------------------------------------------------------- timings

/// How many of a route's latest durations count for its percentiles.
const RECENT: usize = 500;

#[derive(Default)]
struct RouteStats {
    count: u64,
    server_errors: u64,
    total: Duration,
    max: Duration,
    recent: VecDeque<Duration>,
}

static ROUTES: LazyLock<Mutex<HashMap<String, RouteStats>>> = LazyLock::new(Default::default);

/// Middleware for the API's routes: counts each request by its route (such as
/// `GET /api/profiles/{username}/{slug}`) and how long it took.
pub async fn measure(request: Request, next: Next) -> Response {
    let start = Instant::now();
    let route = request
        .extensions()
        .get::<MatchedPath>()
        .map_or("(unmatched)", MatchedPath::as_str);
    let key = format!("{} {route}", request.method());
    let response = next.run(request).await;
    let took = start.elapsed();
    let mut routes = ROUTES.lock().unwrap_or_else(|e| e.into_inner());
    let stats = routes.entry(key).or_default();
    stats.count += 1;
    if response.status().is_server_error() {
        stats.server_errors += 1;
    }
    stats.total += took;
    stats.max = stats.max.max(took);
    if stats.recent.len() == RECENT {
        stats.recent.pop_front();
    }
    stats.recent.push_back(took);
    response
}

#[derive(Serialize)]
struct RouteTiming {
    route: String,
    requests: u64,
    server_errors: u64,
    mean_ms: f64,
    /// Of the latest 500 requests.
    p50_ms: f64,
    p95_ms: f64,
    max_ms: f64,
}

fn ms(d: Duration) -> f64 {
    (d.as_secs_f64() * 10_000.0).round() / 10.0
}

fn timings() -> Vec<RouteTiming> {
    let routes = ROUTES.lock().unwrap_or_else(|e| e.into_inner());
    let mut timings: Vec<_> = routes
        .iter()
        .map(|(route, s)| {
            let mut recent: Vec<_> = s.recent.iter().copied().collect();
            recent.sort();
            let at = |q: f64| {
                recent
                    .get(((recent.len() as f64 * q).ceil() as usize).saturating_sub(1))
                    .copied()
                    .unwrap_or_default()
            };
            RouteTiming {
                route: route.clone(),
                requests: s.count,
                server_errors: s.server_errors,
                mean_ms: ms(s.total / s.count.max(1) as u32),
                p50_ms: ms(at(0.5)),
                p95_ms: ms(at(0.95)),
                max_ms: ms(s.max),
            }
        })
        .collect();
    // The routes that cost the most time in all first.
    timings.sort_by(|a, b| {
        (b.mean_ms * b.requests as f64).total_cmp(&(a.mean_ms * a.requests as f64))
    });
    timings
}

// ---------------------------------------------------------------- process

#[derive(Serialize, Default)]
struct Process {
    pid: u32,
    threads: Option<u64>,
    /// Resident memory now and at its highest.
    memory_bytes: Option<u64>,
    memory_peak_bytes: Option<u64>,
    /// CPU time used, in seconds, and as a share of one core: since the status
    /// was last asked for (within ten minutes), otherwise since the start.
    cpu_seconds: Option<f64>,
    cpu_percent: Option<f64>,
    /// The machine, or the container's limits where it has some.
    cores: usize,
    load_average: Option<[f64; 3]>,
    memory_total_bytes: Option<u64>,
    memory_available_bytes: Option<u64>,
    memory_limit_bytes: Option<u64>,
}

/// Clock ticks per second in /proc/<pid>/stat; 100 on Linux for decades.
const TICKS: f64 = 100.0;

static LAST_CPU: Mutex<Option<(Instant, f64)>> = Mutex::new(None);

fn read(path: &str) -> Option<String> {
    std::fs::read_to_string(path).ok()
}

/// A "Key:   1234 kB" line of /proc/self/status or /proc/meminfo, in bytes.
fn kb_line(text: &str, key: &str) -> Option<u64> {
    text.lines()
        .find_map(|l| l.strip_prefix(key)?.strip_prefix(':'))
        .and_then(|v| v.split_whitespace().next()?.parse::<u64>().ok())
        .map(|kb| kb * 1024)
}

fn process() -> Process {
    let mut p = Process {
        pid: std::process::id(),
        cores: std::thread::available_parallelism().map_or(1, |n| n.get()),
        ..Process::default()
    };
    if let Some(status) = read("/proc/self/status") {
        p.memory_bytes = kb_line(&status, "VmRSS");
        p.memory_peak_bytes = kb_line(&status, "VmHWM");
        p.threads = status
            .lines()
            .find_map(|l| l.strip_prefix("Threads:"))
            .and_then(|v| v.trim().parse().ok());
    }
    // utime and stime are fields 14 and 15; the name in brackets may hold spaces.
    let cpu = read("/proc/self/stat").and_then(|stat| {
        let fields: Vec<&str> = stat.rsplit_once(')')?.1.split_whitespace().collect();
        let ticks = fields.get(11)?.parse::<f64>().ok()? + fields.get(12)?.parse::<f64>().ok()?;
        Some(ticks / TICKS)
    });
    if let Some(seconds) = cpu {
        let now = Instant::now();
        let mut last = LAST_CPU.lock().unwrap_or_else(|e| e.into_inner());
        let (since, before) = match *last {
            Some((at, used))
                if now - at < Duration::from_secs(600) && now - at > Duration::ZERO =>
            {
                (at, used)
            }
            _ => (STARTED.0, 0.0),
        };
        let wall = (now - since).as_secs_f64();
        if wall > 0.0 {
            p.cpu_percent = Some(((seconds - before) / wall * 1000.0).round() / 10.0);
        }
        *last = Some((now, seconds));
        p.cpu_seconds = Some(seconds);
    }
    p.load_average = read("/proc/loadavg").and_then(|l| {
        let mut it = l.split_whitespace().map(|v| v.parse::<f64>().ok());
        Some([it.next()??, it.next()??, it.next()??])
    });
    if let Some(meminfo) = read("/proc/meminfo") {
        p.memory_total_bytes = kb_line(&meminfo, "MemTotal");
        p.memory_available_bytes = kb_line(&meminfo, "MemAvailable");
    }
    // cgroup v2, as in Docker with a memory limit; "max" means none.
    p.memory_limit_bytes = read("/sys/fs/cgroup/memory.max").and_then(|m| m.trim().parse().ok());
    p
}

// ---------------------------------------------------------------- the page

#[derive(Serialize)]
struct Build {
    version: &'static str,
    /// The commit the image was built from, when the build was given one.
    commit: Option<&'static str>,
    debug: bool,
    #[serde(with = "time::serde::rfc3339")]
    started_at: OffsetDateTime,
    uptime_seconds: u64,
}

#[derive(Serialize)]
struct Table {
    name: String,
    rows: i64,
    bytes: i64,
}

#[derive(Serialize)]
struct Recent {
    source: String,
    listens: i64,
}

#[derive(Serialize)]
struct Database {
    version: String,
    bytes: i64,
    /// The latest migration applied.
    migration: Option<i64>,
    pool_size: u32,
    pool_idle: usize,
    connections: Option<i32>,
    /// Share of the blocks read that came from PostgreSQL's cache.
    cache_hit_percent: Option<f64>,
    tables: Vec<Table>,
    /// Listens that came in over the last 24 hours, by source.
    last_day: Vec<Recent>,
}

#[derive(Serialize)]
struct Workers {
    #[serde(with = "time::serde::rfc3339::option")]
    last_round_at: Option<OffsetDateTime>,
    yourspotify: Vec<Connection>,
}

#[derive(Serialize)]
struct Status {
    build: Build,
    process: Process,
    database: Database,
    workers: Workers,
    routes: Vec<RouteTiming>,
}

async fn status(
    State(state): State<AppState>,
    Account(id): Account,
) -> Result<Json<Status>, AppError> {
    let db = &state.db;
    let admin = sqlx::query_scalar!("SELECT is_admin FROM account WHERE id = $1", id)
        .fetch_one(db)
        .await?;
    if !admin {
        return Err(AppError::Status(
            StatusCode::FORBIDDEN,
            "the status is for admins".into(),
        ));
    }

    let overview = sqlx::query!(
        r#"SELECT version() AS "version!",
                  pg_database_size(current_database()) AS "bytes!",
                  (SELECT max(version) FROM _sqlx_migrations WHERE success) AS migration,
                  d.numbackends AS connections,
                  round(100.0 * d.blks_hit / nullif(d.blks_hit + d.blks_read, 0), 1)::float8
                      AS cache_hit_percent
             FROM pg_stat_database d
            WHERE d.datname = current_database()"#
    )
    .fetch_one(db)
    .await?;
    let tables = sqlx::query_as!(
        Table,
        // PostgreSQL's estimates, as counting a big listen table would take a
        // while: the planner's from the last ANALYZE, or the statistics' when
        // those are newer (they start over with a copied database).
        r#"SELECT s.relname::text AS "name!",
                  greatest(s.n_live_tup, c.reltuples::bigint, 0) AS "rows!",
                  pg_total_relation_size(s.relid) AS "bytes!"
             FROM pg_stat_user_tables s
             JOIN pg_class c ON c.oid = s.relid
            WHERE s.relname NOT LIKE '\_sqlx%'
            ORDER BY pg_total_relation_size(s.relid) DESC"#
    )
    .fetch_all(db)
    .await?;
    let last_day = sqlx::query_as!(
        Recent,
        r#"SELECT listen_source(client) AS "source!", count(*) AS "listens!"
             FROM listen
            WHERE submitted_at > now() - interval '1 day'
            GROUP BY 1
            ORDER BY 2 DESC"#
    )
    .fetch_all(db)
    .await?;

    Ok(Json(Status {
        build: Build {
            version: env!("CARGO_PKG_VERSION"),
            commit: option_env!("GIT_COMMIT").filter(|c| !c.is_empty()),
            debug: cfg!(debug_assertions),
            started_at: STARTED.1,
            uptime_seconds: STARTED.0.elapsed().as_secs(),
        },
        process: process(),
        database: Database {
            version: overview.version,
            bytes: overview.bytes,
            migration: overview.migration,
            pool_size: db.size(),
            pool_idle: db.num_idle(),
            connections: overview.connections,
            cache_hit_percent: overview.cache_hit_percent,
            tables,
            last_day,
        },
        workers: Workers {
            last_round_at: connections::last_round(),
            yourspotify: connections::list(db, None).await?,
        },
        routes: timings(),
    }))
}
