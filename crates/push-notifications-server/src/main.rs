use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::Parser;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

use push_notifications_server::driver::mem::MemDb;
use push_notifications_server::fcm_client::RealFcmClient;

#[cfg(feature = "cassandra")]
use push_notifications_server::driver::cassandra::Cassandra;

use push_notifications_server::driver::Driver;

#[derive(Parser)]
#[command(about = "Dash Chat push notifications server")]
struct Cli {
    /// Port to listen on
    #[arg(long, default_value_t = 3000)]
    port: u16,

    /// Path to the Google service account key JSON file
    #[arg(long)]
    service_account_key: PathBuf,
}

#[allow(unused)]
enum DriverType {
    Mem,
    Cassandra,
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

    let driver_type = DriverType::Mem;

    let db: Arc<dyn Driver> = match driver_type {
        DriverType::Mem => Arc::new(MemDb::new()),
        #[cfg(feature = "cassandra")]
        DriverType::Cassandra => {
            let cassandra_url =
                std::env::var("CASSANDRA_URL").unwrap_or_else(|_| "127.0.0.1:9042".to_string());

            tracing::info!("connecting to Cassandra at {cassandra_url}");
            Arc::new(Cassandra::new(&cassandra_url).await?)
        }
        #[cfg(not(feature = "cassandra"))]
        DriverType::Cassandra => {
            anyhow::bail!("cassandra feature is not enabled");
        }
    };

    let addr = format!("0.0.0.0:{}", cli.port);

    tracing::info!(
        "loading FCM credentials from {}",
        cli.service_account_key.display()
    );

    let key_json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&cli.service_account_key)?)
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
