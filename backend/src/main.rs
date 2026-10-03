use std::{env, path::PathBuf};

use anyhow::Context;
use musicbanana::{AppState, router};
use sqlx::postgres::PgPoolOptions;
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "musicbanana=info,tower_http=info".into()),
        )
        .init();

    let database_url = env::var("DATABASE_URL").context("DATABASE_URL is not set")?;
    let listen_addr = env::var("LISTEN_ADDR").unwrap_or_else(|_| "127.0.0.1:3000".into());
    let static_dir =
        PathBuf::from(env::var("STATIC_DIR").unwrap_or_else(|_| "../frontend/build".into()));

    let db = PgPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await
        .context("connecting to PostgreSQL")?;
    sqlx::migrate!()
        .run(&db)
        .await
        .context("running migrations")?;

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
