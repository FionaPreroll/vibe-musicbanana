use std::{
    env,
    io::{self, IsTerminal},
    path::PathBuf,
};

use anyhow::{Context, bail};
use clap::{Parser, Subcommand};
use musicbanana::{AppState, import_php, router, tokens};
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
