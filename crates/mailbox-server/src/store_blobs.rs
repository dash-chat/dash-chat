use axum::{extract::State, http::StatusCode, Json};
use mailbox_api::*;
use redb::Database;

use crate::{blobs_table::BLOBS_TABLE, AppState};

pub async fn store_blobs(
    State(state): State<AppState>,
    Json(payload): Json<StoreBlobsRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let db = state.db.clone();
    // Use spawn_blocking because redb's begin_write() is a blocking call that waits
    // for exclusive write access. Running this directly in async context would block
    // tokio worker threads and cause deadlocks under concurrent load.
    tokio::task::spawn_blocking(move || store_blobs_inner(&db, &payload))
        .await
        .map_err(|e| {
            tracing::error!("Task join error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?
        .map_err(|e| {
            tracing::error!("{}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e)
        })?;
    Ok(StatusCode::CREATED)
}

fn store_blobs_inner(db: &Database, request: &StoreBlobsRequest) -> Result<(), String> {
    let write_txn = db
        .begin_write()
        .map_err(|e| format!("Failed to begin transaction: {}", e))?;

    {
        let mut blobs_table = write_txn
            .open_table(BLOBS_TABLE)
            .map_err(|e| format!("Failed to open blobs table: {}", e))?;

        for blob in &request.blobs {
            let blob_hash = blob.to_hash();
            blobs_table
                .insert(blob_hash, blob.as_ref())
                .map_err(|e| format!("Failed to insert blob: {}", e))?;
        }
    }

    write_txn
        .commit()
        .map_err(|e| format!("Failed to commit transaction: {}", e))?;

    Ok(())
}
