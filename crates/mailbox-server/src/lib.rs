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

mod blip;
mod blips_table;
mod cleanup;
mod get_blips;
mod notify_topics_subscribers;
mod store_blips;
mod watermark;
mod watermarks_table;

#[cfg(feature = "test_utils")]
pub mod test_utils;

// Must comfortably exceed the UI's 16 MiB per-message attachment cap: blips
// arrive base64-encoded in a JSON body (~1.33x the raw bytes, plus operation
// envelope overhead), and one store request can batch several operations.
const MAX_PAYLOAD_SIZE: usize = 64 * 1024 * 1024; // 64 MB

pub use blip::Blip;
pub use blips_table::{BlipsKey, BlipsKeyError, BlipsKeyPrefix, BLIPS_TABLE};
pub use cleanup::{cleanup_old_messages, spawn_cleanup_task};
pub use get_blips::{get_blips_for_topics, GetBlipsRequest, GetBlipsResponse};
pub use store_blips::{store_blips, StoreBlipsRequest};
pub use watermark::compute_initial_watermarks;
pub use watermarks_table::{WatermarksKey, WatermarksKeyError, WATERMARKS_TABLE};

pub type TopicId = String;
pub type Author = String;
pub type SequenceNumber = u64;

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
    push_notifications_url: Option<String>,
    signal: impl Future<Output = ()> + Send + 'static,
) -> Result<(), Box<dyn std::error::Error>> {
    let db = init_db(db_path)?;
    let db_arc = Arc::new(db);

    // Spawn background cleanup task
    let cleanup_task = spawn_cleanup_task(Arc::clone(&db_arc));
    tracing::info!("Started background cleanup task (runs every 5 minutes)");

    let push_client = match push_notifications_url {
        Some(url) => {
            tracing::info!("Push notifications integration enabled: {url}");
            Some(Arc::new(PushNotificationsClient::new(url)?))
        }
        None => None,
    };

    let push_tasks = Arc::new(tokio::sync::Mutex::new(JoinSet::new()));
    let app = create_app(db_arc, push_client, Arc::clone(&push_tasks));

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
        let _blips_table = write_txn.open_table(BLIPS_TABLE)?;
        let _watermarks_table = write_txn.open_table(WATERMARKS_TABLE)?;
    }
    write_txn.commit()?;

    // Compute initial watermarks from existing blips
    compute_initial_watermarks(&db)?;

    tracing::info!("Database initialized successfully");

    Ok(db)
}

pub fn create_app(
    db: Arc<Database>,
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
        .route("/blips/store", post(store_blips))
        .route("/blips/get", post(get_blips_for_topics))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .layer(DefaultBodyLimit::max(MAX_PAYLOAD_SIZE))
        .with_state(state)
}
