use std::sync::Arc;

use anyhow::{Context, Result};
use axum::{Router, routing::post};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

mod driver;
mod error;
mod fcm;
mod routes;
mod types;

use driver::cassandra::Cassandra;
use fcm::FcmClient;

use crate::driver::{Driver, mem::MemDb};

#[derive(Clone)]
pub struct AppState {
    db: Arc<dyn Driver>,
    fcm: Arc<FcmClient>,
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

    let driver_type = DriverType::Mem;

    let db: Arc<dyn Driver> = match driver_type {
        DriverType::Mem => Arc::new(MemDb::new()),
        DriverType::Cassandra => {
            let cassandra_url =
                std::env::var("CASSANDRA_URL").unwrap_or_else(|_| "127.0.0.1:9042".to_string());

            tracing::info!("connecting to Cassandra at {cassandra_url}");
            Arc::new(Cassandra::new(&cassandra_url).await?)
        }
    };

    let addr = std::env::var("ADDR").unwrap_or_else(|_| "0.0.0.0:3000".to_string());

    let service_account_path = std::env::var("GOOGLE_APPLICATION_CREDENTIALS")
        .context("GOOGLE_APPLICATION_CREDENTIALS env var is required")?;

    let project_id =
        std::env::var("FCM_PROJECT_ID").context("FCM_PROJECT_ID env var is required")?;

    tracing::info!("loading FCM credentials for project {project_id}");
    let fcm = FcmClient::new(&service_account_path, project_id).await?;

    let state = AppState {
        db,
        fcm: Arc::new(fcm),
    };

    let app = Router::new()
        .route("/fcm-token", post(routes::store_token::store_token))
        .route("/push", post(routes::send_push::send_push))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .with_context(|| format!("failed to bind to {addr}"))?;

    tracing::info!("listening on {addr}");
    axum::serve(listener, app).await?;

    Ok(())
}
