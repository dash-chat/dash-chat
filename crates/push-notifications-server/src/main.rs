use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::Parser;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

use push_notifications_server::driver::mem::MemDb;
use push_notifications_server::driver::sql::SqlDriver;
use push_notifications_server::fcm_client::RealFcmClient;

use push_notifications_server::driver::Driver;

#[derive(Parser)]
#[command(about = "Dash Chat push notifications server")]
struct Cli {
    /// Address to bind the server to
    #[arg(short, long, default_value = "0.0.0.0:3000")]
    addr: String,

    /// Path to the Google service account key JSON file
    #[arg(long)]
    service_account_key: PathBuf,

    /// Path to the SQLite database file
    #[arg(long, conflicts_with = "mem")]
    db_path: Option<PathBuf>,

    /// Use an in-memory database instead of SQLite
    #[arg(long, conflicts_with = "db_path")]
    mem: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let cli = Cli::parse();

    anyhow::ensure!(
        cli.service_account_key.exists(),
        "service account key file not found: {}",
        cli.service_account_key.display()
    );

    let db: Arc<dyn Driver> = if cli.mem {
        tracing::info!("using in-memory database");
        Arc::new(MemDb::new())
    } else {
        sqlx::any::install_default_drivers();

        let db_path = cli
            .db_path
            .as_deref()
            .context("--db-path is required when --mem is not set")?;
        // Create parent directory if it does not exist
        if let Some(parent) = db_path.parent().filter(|p| !p.exists()) {
            std::fs::create_dir_all(parent)?;
        }
        let url = format!("sqlite:{}?mode=rwc", db_path.display());
        tracing::info!("opening database at {}", db_path.display());
        Arc::new(SqlDriver::new(&url).await?)
    };

    let addr = &cli.addr;

    tracing::info!("loading FCM credentials from {}", cli.service_account_key.display());

    let key_json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&cli.service_account_key)?)
        .context("failed to parse service account key JSON")?;

    let project_id = key_json["project_id"]
        .as_str()
        .context("service account key JSON missing 'project_id' field")?;

    let fcm = RealFcmClient::new(&cli.service_account_key, project_id).await?;

    let app = push_notifications_server::build(db, Arc::new(fcm)).await?;

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .with_context(|| format!("failed to bind to {addr}"))?;

    tracing::info!("listening on {addr}");
    axum::serve(listener, app).await?;

    Ok(())
}
