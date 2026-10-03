use std::{env, path::PathBuf};

use anyhow::Context;
use clap::{Parser, Subcommand};
use musicbanana::{AppState, import_php, router};
use sqlx::{PgPool, postgres::PgPoolOptions};
use tokio::net::TcpListener;
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
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "musicbanana=info,tower_http=info".into()),
        )
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
    }
}

async fn serve(db: PgPool) -> anyhow::Result<()> {
    let listen_addr = env::var("LISTEN_ADDR").unwrap_or_else(|_| "127.0.0.1:3000".into());
    let static_dir =
        PathBuf::from(env::var("STATIC_DIR").unwrap_or_else(|_| "../frontend/build".into()));

    let app = router(AppState { db }, &static_dir);
    let listener = TcpListener::bind(&listen_addr).await?;
    tracing::info!("listening on http://{listen_addr}");
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            tokio::signal::ctrl_c().await.ok();
        })
        .await?;
    Ok(())
}
