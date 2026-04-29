use axum::{
    extract::DefaultBodyLimit,
    routing::{get, post},
    Json, Router,
};
use push_notifications_client::client::PushNotificationsClient;
use redb::Database;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::{future::Future, path::PathBuf};
use tokio::task::JoinSet;
use tower_http::{cors::CorsLayer, trace::TraceLayer};

mod cleanup;
mod dollops_table;
pub mod dump;
mod get_dollops;
mod notify_topics_subscribers;
mod store_dollops;
mod watermark;
mod watermarks_table;

mod blobs_table;
mod get_blobs;
mod store_blobs;
#[cfg(feature = "test_utils")]
pub mod test_utils;

pub use blobs_table::BLOBS_TABLE;
pub use cleanup::{cleanup_old_messages, spawn_cleanup_task};
pub use dollops_table::{DollopsKey, DollopsKeyError, DollopsKeyPrefix, DOLLOPS_TABLE};
pub use get_blobs::get_blobs;
pub use get_dollops::get_dollops_for_topics;
pub use mailbox_api::{
    Author, GetDollopsRequest, GetDollopsResponse, Opaq, SequenceNumber, StoreDollopsRequest,
    TopicId,
};
pub use store_blobs::store_blobs;
pub use store_dollops::store_dollops;
pub use watermark::compute_initial_watermarks;
pub use watermarks_table::{WatermarksKey, WatermarksKeyError, WATERMARKS_TABLE};

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Database>,
    pub push_client: Option<Arc<PushNotificationsClient>>,
    pub push_tasks: Arc<tokio::sync::Mutex<JoinSet<()>>>,
}

#[derive(Serialize, Deserialize)]
struct HealthResponse {
    status: String,
}

pub async fn spawn_server(
    db_path: PathBuf,
    addr: String,
    payload_max_size_mb: usize,
    data_max_age_days: u64,
    push_notifications_url: Option<String>,
    signal: impl Future<Output = ()> + Send + 'static,
) -> Result<(), Box<dyn std::error::Error>> {
    let db = init_db(db_path)?;
    let db_arc = Arc::new(db);

    // Spawn background cleanup task
    let cleanup_task = spawn_cleanup_task(Arc::clone(&db_arc), data_max_age_days);
    tracing::info!("Started background cleanup task (runs every 5 minutes)");

    let push_client = match push_notifications_url {
        Some(url) => {
            tracing::info!("Push notifications integration enabled: {url}");
            Some(Arc::new(PushNotificationsClient::new(url)?))
        }
        None => None,
    };

    let push_tasks = Arc::new(tokio::sync::Mutex::new(JoinSet::new()));
    let app = create_app(
        db_arc,
        payload_max_size_mb,
        push_client,
        Arc::clone(&push_tasks),
    );

    let listener = tokio::net::TcpListener::bind(addr).await?;
    let addr = listener.local_addr()?;

    tracing::info!("Mailbox server listening on {}", addr);

    let server = axum::serve(listener, app);
    server.with_graceful_shutdown(signal).await?;

    // TODO: cleanup task needs to be cleaned up even if the server is aborted.
    //      the database stays open as long as this task holds a reference to the db arc.

    // Drain pending push notification tasks before shutting down
    let mut tasks = push_tasks.lock().await;
    while tasks.join_next().await.is_some() {}

    cleanup_task.abort();
    tracing::info!("Mailbox server gracefully shut down");

    Ok(())
}

async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
    })
}

pub fn init_db(db_path: PathBuf) -> Result<Database, Box<dyn std::error::Error>> {
    tracing::info!("Opening redb database at {:?}", db_path);

    if let Some(parent) = db_path.parent().filter(|p| !p.exists()) {
        std::fs::create_dir_all(parent)?;
    }

    let db = Database::create(&db_path)?;

    let write_txn = db.begin_write()?;
    {
        let _dollops_table = write_txn.open_table(DOLLOPS_TABLE)?;
        let _blobs_table = write_txn.open_table(BLOBS_TABLE)?;
        let _watermarks_table = write_txn.open_table(WATERMARKS_TABLE)?;
    }
    write_txn.commit()?;

    // Compute initial watermarks from existing dollops
    compute_initial_watermarks(&db)?;

    tracing::info!("Database initialized successfully");

    Ok(db)
}

pub fn create_app(
    db: Arc<Database>,
    payload_max_size_mb: usize,
    push_client: Option<Arc<PushNotificationsClient>>,
    push_tasks: Arc<tokio::sync::Mutex<JoinSet<()>>>,
) -> Router {
    let state = AppState {
        db,
        push_client,
        push_tasks,
    };

    Router::new()
        .route("/health", get(health_check))
        .route("/dollops/store", post(store_dollops))
        .route("/dollops/get", post(get_dollops_for_topics))
        .route("/dollops/dump", get(dump::dump_dollops))
        .route("/blobs/get", post(get_blobs))
        .route("/blobs/store", post(store_blobs))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .layer(DefaultBodyLimit::max(1024 * 1024 * payload_max_size_mb))
        .with_state(state)
}
