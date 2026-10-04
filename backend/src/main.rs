use std::{
    env,
    io::{self, IsTerminal},
    path::PathBuf,
};

use anyhow::{Context, bail};
use clap::{Args, Parser, Subcommand, ValueEnum};
use musicbanana::{
    AppState, import_php,
    merge::{self, Kind, Suggestion},
    router, tokens,
};
use sqlx::{PgPool, postgres::PgPoolOptions};
use time::OffsetDateTime;
use tokio::{
    net::TcpListener,
    signal::unix::{SignalKind, signal},
};
use tracing_subscriber::EnvFilter;

/// musicbanana server. Configured through DATABASE_URL, LISTEN_ADDR and STATIC_DIR.
#[derive(Parser)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Run the web server (default).
    Serve,
    /// Import a musicbanana-php database into an empty musicbanana database.
    ImportPhp {
        /// MySQL/MariaDB URL of the old database, e.g. mysql://root@127.0.0.1:3306/musicbanana
        #[arg(long)]
        from: String,
    },
    /// Manage the tokens scrobble clients use (ListenBrainz API).
    #[command(subcommand)]
    Token(TokenCommand),
    /// Merge duplicates in the catalog, e.g. two spellings of an artist.
    #[command(subcommand)]
    Merge(MergeCommand),
}

#[derive(Subcommand)]
enum TokenCommand {
    /// Create a token; it is printed once and cannot be shown again.
    Create {
        /// Account the token is for.
        #[arg(long)]
        user: String,
        /// Profile the listens go to.
        #[arg(long, default_value = "default")]
        profile: String,
        /// What the token is used by, e.g. "Navidrome".
        #[arg(long)]
        label: String,
    },
    /// List tokens, of all accounts or of one.
    List {
        #[arg(long)]
        user: Option<String>,
    },
    /// Revoke a token; clients using it are refused from then on.
    Revoke {
        /// Token id, as shown by `token list`.
        id: i64,
    },
}

#[derive(Subcommand)]
enum MergeCommand {
    /// List artists, releases and recordings that look like another one, each
    /// with the command that merges them.
    Suggest {
        /// Only these. Artists are best merged first: that also merges their
        /// releases and recordings with the same title.
        #[arg(value_enum)]
        kind: Option<Kinds>,
        /// Suggestions shown per kind.
        #[arg(long, default_value_t = 30)]
        limit: usize,
    },
    /// Merge an artist into another one, with its releases and recordings.
    Artist(MergeArgs),
    /// Merge a release (album) into another one.
    Release(MergeArgs),
    /// Merge a recording (track) into another one.
    Recording(MergeArgs),
}

#[derive(Args)]
struct MergeArgs {
    /// Id of the entry that goes away, as shown by `merge suggest`.
    from: i64,
    /// Id of the entry that stays.
    into: i64,
    /// Only show what would change.
    #[arg(long)]
    dry_run: bool,
    /// Merge even though their MusicBrainz IDs tell the two apart, such as an
    /// artist's other name into the main one, or a recording MusicBrainz lists twice.
    #[arg(long)]
    force: bool,
}

#[derive(Clone, Copy, ValueEnum)]
enum Kinds {
    Artists,
    Releases,
    Recordings,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "musicbanana=info,tower_http=info".into()),
        )
        // No color codes in `docker compose logs` and other redirected output.
        .with_ansi(io::stdout().is_terminal())
        .init();
    let cli = Cli::parse();

    let database_url = env::var("DATABASE_URL").context("DATABASE_URL is not set")?;
    let db = PgPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await
        .context("connecting to PostgreSQL")?;
    sqlx::migrate!()
        .run(&db)
        .await
        .context("running migrations")?;

    match cli.command.unwrap_or(Command::Serve) {
        Command::Serve => serve(db).await,
        Command::ImportPhp { from } => {
            let data = import_php::read_mysql(&from).await?;
            let report = import_php::write(&db, &data).await?;
            println!("{report}");
            Ok(())
        }
        Command::Token(command) => token(&db, command).await,
        Command::Merge(command) => merge_command(&db, command).await,
    }
}

async fn token(db: &PgPool, command: TokenCommand) -> anyhow::Result<()> {
    match command {
        TokenCommand::Create {
            user,
            profile,
            label,
        } => {
            let new = tokens::create(db, &user, &profile, &label).await?;
            // Only the token goes to stdout, so it can be piped somewhere.
            eprintln!(
                "Token {} for {user}/{profile} ({label}), shown only this once:",
                new.id
            );
            println!("{}", new.token);
        }
        TokenCommand::List { user } => {
            let list = tokens::list(db, user.as_deref()).await?;
            if list.is_empty() {
                eprintln!("No tokens.");
            }
            for t in list {
                let used = t.last_used_at.map_or("never used".into(), |at| {
                    format!("last used {}", minutes(at))
                });
                let revoked = t
                    .revoked_at
                    .map_or(String::new(), |at| format!(", revoked {}", minutes(at)));
                println!(
                    "{:>4}  {}/{}  {}  created {}, {used}{revoked}",
                    t.id,
                    t.username,
                    t.slug,
                    t.label,
                    minutes(t.created_at)
                );
            }
        }
        TokenCommand::Revoke { id } => {
            if !tokens::revoke(db, id).await? {
                bail!("there is no active token {id}");
            }
            eprintln!("Token {id} revoked.");
        }
    }
    Ok(())
}

async fn merge_command(db: &PgPool, command: MergeCommand) -> anyhow::Result<()> {
    let (kind, args) = match command {
        MergeCommand::Suggest { kind, limit } => {
            let kinds = match kind {
                None => vec![Kind::Artist, Kind::Release, Kind::Recording],
                Some(Kinds::Artists) => vec![Kind::Artist],
                Some(Kinds::Releases) => vec![Kind::Release],
                Some(Kinds::Recordings) => vec![Kind::Recording],
            };
            for kind in kinds {
                let found = merge::suggest(db, kind).await?;
                print_suggestions(kind, &found, limit);
            }
            return Ok(());
        }
        MergeCommand::Artist(args) => (Kind::Artist, args),
        MergeCommand::Release(args) => (Kind::Release, args),
        MergeCommand::Recording(args) => (Kind::Recording, args),
    };
    let options = merge::Options {
        dry_run: args.dry_run,
        force: args.force,
    };
    let merged = merge::merge(db, kind, args.from, args.into, options).await?;
    println!("{merged}");
    Ok(())
}

/// One line per suggestion: the command, then why as a shell comment, so the
/// whole line can be pasted.
fn print_suggestions(kind: Kind, found: &[Suggestion], limit: usize) {
    let heading = match kind {
        Kind::Artist => "Artists",
        Kind::Release => "Releases",
        Kind::Recording => "Recordings",
    };
    if found.is_empty() {
        println!("{heading} that look alike: none\n");
        return;
    }
    println!("{heading} that look alike, most certain first:");
    let shown = &found[..found.len().min(limit)];
    let commands: Vec<String> = shown
        .iter()
        .map(|s| format!("musicbanana merge {kind} {} {}", s.from.id, s.into.id))
        .collect();
    let width = commands.iter().map(String::len).max().unwrap_or(0);
    for (s, command) in shown.iter().zip(&commands) {
        let by = s
            .into
            .artist
            .as_ref()
            .map_or(String::new(), |artist| format!(", by {artist}"));
        println!(
            "{command:width$}  # {}: \"{}\" ({}) into \"{}\" ({}){by}",
            s.likeness,
            s.from.name,
            listens(s.from.listens),
            s.into.name,
            listens(s.into.listens),
        );
    }
    if found.len() > shown.len() {
        println!(
            "… and {} more, see --limit {}",
            found.len() - shown.len(),
            found.len()
        );
    }
    println!();
}

fn listens(n: i64) -> String {
    if n == 1 {
        "1 listen".to_owned()
    } else {
        format!("{n} listens")
    }
}

/// "2026-10-03 14:30 UTC"
fn minutes(at: OffsetDateTime) -> String {
    let at = at.to_offset(time::UtcOffset::UTC);
    format!("{} {:02}:{:02} UTC", at.date(), at.hour(), at.minute())
}

async fn serve(db: PgPool) -> anyhow::Result<()> {
    let listen_addr = env::var("LISTEN_ADDR").unwrap_or_else(|_| "127.0.0.1:3000".into());
    let static_dir =
        PathBuf::from(env::var("STATIC_DIR").unwrap_or_else(|_| "../frontend/build".into()));

    let app = router(AppState { db }, &static_dir);
    let listener = TcpListener::bind(&listen_addr).await?;
    tracing::info!("listening on http://{listen_addr}");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

/// Ctrl+C, or SIGTERM from `docker stop`. In a container the server runs as
/// PID 1, which gets no default handler for SIGTERM and would otherwise only
/// stop when Docker kills it after the grace period.
async fn shutdown_signal() {
    let terminate = async {
        match signal(SignalKind::terminate()) {
            Ok(mut sigterm) => {
                sigterm.recv().await;
            }
            Err(_) => std::future::pending().await,
        }
    };
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        () = terminate => {}
    }
    tracing::info!("shutting down");
}
