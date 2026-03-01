use axum::{extract::State, http::StatusCode, Json};
use mailbox_api::*;
use redb::{Database, ReadableDatabase, ReadableTable};
use std::collections::BTreeSet;

use crate::{blobs_table::BLOBS_TABLE, AppState};

pub async fn get_blobs(
    State(state): State<AppState>,
    Json(payload): Json<GetBlobsRequest>,
) -> Result<Json<GetBlobsResponse>, (StatusCode, String)> {
    let db = state.db.clone();
    // Use spawn_blocking because redb's begin_write() is a blocking call that waits
    // for exclusive write access. Running this directly in async context would block
    // tokio worker threads and cause deadlocks under concurrent load.
    tokio::task::spawn_blocking(move || get_blobs_inner(&db, &payload))
        .await
        .map_err(|e| {
            tracing::error!("Task join error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?
        .map(Json)
        .map_err(|e| {
            tracing::error!("{}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e)
        })
}

fn get_blobs_inner(db: &Database, request: &GetBlobsRequest) -> Result<GetBlobsResponse, String> {
    let txn = db
        .begin_read()
        .map_err(|e| format!("Failed to begin transaction: {}", e))?;

    let mut blobs = Vec::new();

    {
        let blobs_table = txn
            .open_table(BLOBS_TABLE)
            .map_err(|e| format!("Failed to open blobs table: {}", e))?;

        for blob_hash in &request.blob_hashes {
            let blob = blobs_table
                .get(blob_hash)
                .map_err(|e| format!("Failed to get blob: {}", e))?;
            if let Some(blob) = blob {
                blobs.push(Opaq::from(blob.value().to_vec()));
            }
        }
    }

    Ok(GetBlobsResponse { blobs })
}
