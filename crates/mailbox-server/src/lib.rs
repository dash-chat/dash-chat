use axum::{
    routing::{get, post},
    Json, Router,
};
use redb::Database;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::{future::Future, path::PathBuf};
use tower_http::{cors::CorsLayer, trace::TraceLayer};

mod blob;
mod blobs_table;
mod cleanup;
mod get_blobs;
mod store_blobs;
mod watermark;
mod watermarks_table;

#[cfg(feature = "test_utils")]
pub mod test_utils;

pub use blob::Blob;
pub use blobs_table::{BlobsKey, BlobsKeyError, BlobsKeyPrefix, BLOBS_TABLE};
pub use cleanup::{cleanup_old_messages, spawn_cleanup_task};
pub use get_blobs::{get_blobs_for_topics, GetBlobsRequest, GetBlobsResponse};
pub use store_blobs::{store_blobs, StoreBlobsRequest};
pub use watermark::compute_initial_watermarks;
pub use watermarks_table::{WatermarksKey, WatermarksKeyError, WATERMARKS_TABLE};

pub type TopicId = String;
pub type Author = String;
pub type SequenceNumber = u64;

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Database>,
}

#[derive(Serialize, Deserialize)]
struct HealthResponse {
    status: String,
}

pub async fn spawn_server(
    db_path: PathBuf,
    addr: String,
    signal: impl Future<Output = ()> + Send + 'static,
) -> Result<(), Box<dyn std::error::Error>> {
    let db = init_db(db_path)?;
    let db_arc = Arc::new(db);

    // Spawn background cleanup task
    let cleanup_task = spawn_cleanup_task(Arc::clone(&db_arc));
    tracing::info!("Started background cleanup task (runs every 5 minutes)");

    let app = create_app_with_arc(db_arc);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    let addr = listener.local_addr()?;

    tracing::info!("Mailbox server listening on {}", addr);

    let server = axum::serve(listener, app);
    server.with_graceful_shutdown(signal).await?;
    // TODO: cleanup task needs to be cleaned up even if the server is aborted.
    //      the database stays open as long as this task holds a reference to the db arc.
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

    let db = Database::create(&db_path)?;

    let write_txn = db.begin_write()?;
    {
        let _blobs_table = write_txn.open_table(BLOBS_TABLE)?;
        let _watermarks_table = write_txn.open_table(WATERMARKS_TABLE)?;
    }
    write_txn.commit()?;

    // Compute initial watermarks from existing blobs
    compute_initial_watermarks(&db)?;

    tracing::info!("Database initialized successfully");

    Ok(db)
}

pub fn create_app(db: Database) -> Router {
    create_app_with_arc(Arc::new(db))
}

pub fn create_app_with_arc(db: Arc<Database>) -> Router {
    let state = AppState { db };

    Router::new()
        .route("/health", get(health_check))
        .route("/blobs/store", post(store_blobs))
        .route("/blobs/get", post(get_blobs_for_topics))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
