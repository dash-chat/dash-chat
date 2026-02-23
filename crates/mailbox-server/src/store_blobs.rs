use axum::{extract::State, http::StatusCode, Json};
use redb::{Database, ReadableTable};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

use crate::{
    AppState, Author, Blob, BlobsKey, BlobsKeyPrefix, SequenceNumber, TopicId, WatermarksKey,
    BLOBS_TABLE, WATERMARKS_TABLE,
};

#[derive(Serialize, Deserialize)]
pub struct StoreBlobsRequest {
    pub blobs: BTreeMap<TopicId, BTreeMap<Author, BTreeMap<SequenceNumber, Blob>>>,
}

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

    let mut blob_count = 0;

    {
        let mut blobs_table = write_txn
            .open_table(BLOBS_TABLE)
            .map_err(|e| format!("Failed to open blobs table: {}", e))?;

        let mut watermarks_table = write_txn
            .open_table(WATERMARKS_TABLE)
            .map_err(|e| format!("Failed to open watermarks table: {}", e))?;

        for (topic_id, authors) in &request.blobs {
            for (author, sequences) in authors {
                let watermarks_key = WatermarksKey::new(topic_id.clone(), author.clone())
                    .map_err(|e| e.to_string())?;

                // Get current watermark for this topic:author
                let current_watermark = watermarks_table
                    .get(&watermarks_key)
                    .map_err(|e| format!("Failed to read watermark: {}", e))?
                    .map(|v| v.value());

                // Collect sequence numbers being stored (BTreeMap is already sorted)
                let mut stored_seqs: BTreeSet<SequenceNumber> = BTreeSet::new();

                for (seq_num, blob) in sequences {
                    let key = BlobsKey::new_now(topic_id.clone(), author.clone(), *seq_num)
                        .map_err(|e| e.to_string())?;

                    blobs_table
                        .insert(&key, blob.as_slice())
                        .map_err(|e| format!("Failed to insert blob: {}", e))?;
                    stored_seqs.insert(*seq_num);
                    blob_count += 1;
                }

                // Update watermark for this topic:author
                let new_watermark = compute_new_watermark(
                    &blobs_table,
                    topic_id,
                    author,
                    current_watermark,
                    &stored_seqs,
                )?;

                if let Some(wm) = new_watermark {
                    // Only update if watermark changed or was newly established
                    if current_watermark != Some(wm) {
                        watermarks_table
                            .insert(&watermarks_key, wm)
                            .map_err(|e| format!("Failed to update watermark: {}", e))?;
                        tracing::debug!(
                            "Updated watermark for {}:{} from {:?} to {}",
                            topic_id,
                            author,
                            current_watermark,
                            wm
                        );
                    }
                }
            }
        }
    }

    write_txn
        .commit()
        .map_err(|e| format!("Failed to commit transaction: {}", e))?;

    tracing::debug!("Stored {} blobs", blob_count);
    Ok(())
}

/// Computes the new watermark after storing blobs.
/// Returns None if no watermark can be established (no sequence 0).
fn compute_new_watermark(
    blobs_table: &redb::Table<BlobsKey, &[u8]>,
    topic_id: &str,
    author: &str,
    current_watermark: Option<SequenceNumber>,
    new_sequences: &BTreeSet<SequenceNumber>,
) -> Result<Option<SequenceNumber>, String> {
    // watermark = None means we need to check from seq 0
    // watermark = Some(n) means seqs 0..=n are confirmed present
    let mut watermark: Option<SequenceNumber> = match current_watermark {
        Some(current_wm) => {
            // Check if new sequences or existing blobs don't extend current watermark
            if !new_sequences.contains(&(current_wm + 1))
                && !blob_exists(blobs_table, topic_id, author, current_wm + 1)?
            {
                return Ok(Some(current_wm)); // No extension possible
            }
            Some(current_wm)
        }
        None => {
            // No watermark yet - need sequence 0 to start
            if !new_sequences.contains(&0) && !blob_exists(blobs_table, topic_id, author, 0)? {
                return Ok(None); // Can't establish watermark without seq 0
            }
            None // Start from None, first iteration will check seq 0
        }
    };

    // Extend watermark by checking consecutive sequences
    loop {
        let next_seq = watermark.map_or(0, |w| w + 1);

        // First check new sequences (cheaper), then existing blobs
        if new_sequences.contains(&next_seq)
            || blob_exists(blobs_table, topic_id, author, next_seq)?
        {
            watermark = Some(next_seq);
        } else {
            break;
        }
    }

    Ok(watermark)
}

/// Checks if a blob exists for the given topic:author:seq
fn blob_exists(
    table: &redb::Table<BlobsKey, &[u8]>,
    topic_id: &str,
    author: &str,
    seq_num: SequenceNumber,
) -> Result<bool, String> {
    let prefix = BlobsKeyPrefix::TopicAuthorSeq(topic_id.to_string(), author.to_string(), seq_num);

    // Use range query to check if any blob exists for this topic:author:seq
    let mut iter = table
        .range(prefix.range_start()..=prefix.range_end())
        .map_err(|e| format!("Failed to create iterator: {}", e))?;

    Ok(iter.next().is_some())
}
