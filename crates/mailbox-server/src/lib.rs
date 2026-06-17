use axum::{
    extract::{DefaultBodyLimit, State},
    routing::{get, post},
    Json, Router,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use push_notifications_client::client::PushNotificationsClient;
use redb::Database;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::{future::Future, path::PathBuf};
use tokio::task::JoinSet;
use tower_http::{cors::CorsLayer, trace::TraceLayer};

mod blob_sync;
mod blip;
mod blips_table;
mod server_key;
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

pub use blob_sync::{BlobSync, BlobFetchPool};
pub use blip::Blip;
pub use blips_table::{BlipsKey, BlipsKeyError, BlipsKeyPrefix, BLIPS_TABLE};
pub use cleanup::{cleanup_old_messages, spawn_cleanup_task};
pub use get_blips::{get_blips_for_topics, GetBlipsRequest, GetBlipsResponse, GetBlipsForTopicResponse};
pub use store_blips::{store_blips, StoreBlipsRequest};
pub use watermark::compute_initial_watermarks;
pub use server_key::{load_or_create_secret_key, SERVER_KEY_TABLE};
pub use watermarks_table::{WatermarksKey, WatermarksKeyError, WATERMARKS_TABLE};

pub type TopicId = String;
pub type Author = String;
pub type SequenceNumber = u64;

/// Encode an iroh EndpointId as the canonical MailboxId string (base64url, no pad).
pub fn encode_mailbox_id(id: iroh::EndpointId) -> String {
    URL_SAFE_NO_PAD.encode(id.as_bytes())
}

/// Parse a MailboxId string back into an iroh EndpointId.
pub fn decode_mailbox_id(s: &str) -> anyhow::Result<iroh::EndpointId> {
    let bytes = URL_SAFE_NO_PAD.decode(s)?;
    let arr: [u8; 32] = bytes
        .as_slice()
        .try_into()
        .map_err(|_| anyhow::anyhow!("MailboxId is not 32 bytes"))?;
    Ok(iroh::EndpointId::from_bytes(&arr)?)
}

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Database>,
    pub push_client: Option<Arc<PushNotificationsClient>>,
    pub push_tasks: Arc<tokio::sync::Mutex<JoinSet<()>>>,
    pub blob_sync: BlobSync,
}

#[derive(Serialize, Deserialize)]
struct HealthResponse {
    status: String,
    endpoint_id: String,
}

fn db_path_blobs_dir(db_path: &std::path::Path) -> std::path::PathBuf {
    db_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .join("mailbox_blobs")
}

pub async fn spawn_server(
    db_path: PathBuf,
    addr: String,
    push_notifications_url: Option<String>,
    signal: impl Future<Output = ()> + Send + 'static,
) -> Result<(), Box<dyn std::error::Error>> {
    let db = init_db(db_path.clone())?;
    let db_arc = Arc::new(db);

    // Spawn background cleanup task
    let cleanup_task = spawn_cleanup_task(Arc::clone(&db_arc));
    tracing::info!("Started background cleanup task (runs every 5 minutes)");

    let secret_key = load_or_create_secret_key(&db_arc).map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
    let blobs_root = db_path_blobs_dir(&db_path);
    let blob_sync = BlobSync::new(secret_key, blobs_root).await?;
    tracing::info!("Mailbox iroh endpoint id: {}", blob_sync.endpoint_id());
    let blob_fetch_handle = blob_sync.spawn_fetch_loop(dashchat_utils::FetchConfig::default());

    let push_client = match push_notifications_url {
        Some(url) => {
            tracing::info!("Push notifications integration enabled: {url}");
            Some(Arc::new(PushNotificationsClient::new(url)?))
        }
        None => None,
    };

    let push_tasks = Arc::new(tokio::sync::Mutex::new(JoinSet::new()));
    let app = create_app(db_arc, push_client, Arc::clone(&push_tasks), blob_sync);

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
    blob_fetch_handle.abort();
    tracing::info!("Mailbox server gracefully shut down");

    Ok(())
}

async fn health_check(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        endpoint_id: encode_mailbox_id(state.blob_sync.endpoint_id()),
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
        let _server_key_table = write_txn.open_table(SERVER_KEY_TABLE)?;
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
    blob_sync: BlobSync,
) -> Router {
    let state = AppState {
        db,
        push_client,
        push_tasks,
        blob_sync,
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
