use axum::{extract::State, http::StatusCode, Json};
use redb::{Database, ReadableTable};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

use crate::{
    notify_topics_subscribers::notify_topics_subscribers, AppState, Author, Blip, BlipsKey,
    BlipsKeyPrefix, BlobSync, SequenceNumber, TopicId, WatermarksKey, BLIPS_TABLE,
    WATERMARKS_TABLE,
};

#[derive(Serialize, Deserialize)]
pub struct StoreBlipsRequest {
    pub blips: BTreeMap<TopicId, BTreeMap<Author, BTreeMap<SequenceNumber, Blip>>>,
    #[serde(default)]
    pub blob_hashes: Vec<iroh_blobs::Hash>,
    #[serde(default)]
    pub sender_pubkey: Option<iroh::EndpointId>,
    #[serde(default)]
    pub signature: Vec<u8>,
}

pub async fn record_blob_sources(
    blob_sync: &BlobSync,
    hashes: &[iroh_blobs::Hash],
    source: iroh::EndpointId,
) {
    for hash in hashes {
        blob_sync.fetch_pool().add_source(*hash, source).await;
    }
}

pub async fn store_blips(
    State(state): State<AppState>,
    Json(payload): Json<StoreBlipsRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let db = state.db.clone();
    let blob_hashes = payload.blob_hashes.clone();
    let sender_pubkey = payload.sender_pubkey;
    // Use spawn_blocking because redb's begin_write() is a blocking call that waits
    // for exclusive write access. Running this directly in async context would block
    // tokio worker threads and cause deadlocks under concurrent load.
    let topics_with_new_blips =
        tokio::task::spawn_blocking(move || store_blips_inner(&db, &payload))
            .await
            .map_err(|e| {
                tracing::error!("Task join error: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".to_string(),
                )
            })?
            .map_err(|e| {
                tracing::error!("{}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".to_string(),
                )
            })?;

    if let Some(pubkey) = sender_pubkey {
        record_blob_sources(&state.blob_sync, &blob_hashes, pubkey).await;
    }
    notify_topics_subscribers(&state, topics_with_new_blips).await;

    Ok(StatusCode::CREATED)
}

/// Returns a map of topic_id → map of op_id (author:seq) → author for newly inserted blips.
/// The author is preserved separately so the push-notifications-server can filter the
/// author out of the subscriber list (devices don't get pushes for their own messages).
fn store_blips_inner(
    db: &Database,
    request: &StoreBlipsRequest,
) -> Result<BTreeMap<TopicId, BTreeMap<String, Author>>, String> {
    let write_txn = db
        .begin_write()
        .map_err(|e| format!("Failed to begin transaction: {}", e))?;

    let mut blip_count = 0;
    let mut topics_with_new_blips: BTreeMap<TopicId, BTreeMap<String, Author>> = BTreeMap::new();

    {
        let mut blips_table = write_txn
            .open_table(BLIPS_TABLE)
            .map_err(|e| format!("Failed to open blips table: {}", e))?;

        let mut watermarks_table = write_txn
            .open_table(WATERMARKS_TABLE)
            .map_err(|e| format!("Failed to open watermarks table: {}", e))?;

        for (topic_id, authors) in &request.blips {
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

                for (seq_num, blip) in sequences {
                    let key = BlipsKey::new_now(topic_id.clone(), author.clone(), *seq_num)
                        .map_err(|e| e.to_string())?;

                    blips_table
                        .insert(&key, blip.as_slice())
                        .map_err(|e| format!("Failed to insert blip: {}", e))?;
                    stored_seqs.insert(*seq_num);
                    blip_count += 1;
                }

                // Update watermark for this topic:author
                let new_watermark = compute_new_watermark(
                    &blips_table,
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
                        let topic_entry =
                            topics_with_new_blips.entry(topic_id.clone()).or_default();
                        for seq in &stored_seqs {
                            topic_entry.insert(format!("{}:{}", author, seq), author.clone());
                        }
                    }
                }
            }
        }
    }

    write_txn
        .commit()
        .map_err(|e| format!("Failed to commit transaction: {}", e))?;

    tracing::debug!("Stored {} blips", blip_count);
    Ok(topics_with_new_blips)
}

/// Computes the new watermark after storing blips.
/// Returns None if no watermark can be established (no sequence 0).
fn compute_new_watermark(
    blips_table: &redb::Table<BlipsKey, &[u8]>,
    topic_id: &str,
    author: &str,
    current_watermark: Option<SequenceNumber>,
    new_sequences: &BTreeSet<SequenceNumber>,
) -> Result<Option<SequenceNumber>, String> {
    // watermark = None means we need to check from seq 0
    // watermark = Some(n) means seqs 0..=n are confirmed present
    let mut watermark: Option<SequenceNumber> = match current_watermark {
        Some(current_wm) => {
            // Check if new sequences or existing blips don't extend current watermark
            if !new_sequences.contains(&(current_wm + 1))
                && !blip_exists(blips_table, topic_id, author, current_wm + 1)?
            {
                return Ok(Some(current_wm)); // No extension possible
            }
            Some(current_wm)
        }
        None => {
            // No watermark yet - need sequence 0 to start
            if !new_sequences.contains(&0) && !blip_exists(blips_table, topic_id, author, 0)? {
                return Ok(None); // Can't establish watermark without seq 0
            }
            None // Start from None, first iteration will check seq 0
        }
    };

    // Extend watermark by checking consecutive sequences
    loop {
        let next_seq = watermark.map_or(0, |w| w + 1);

        // First check new sequences (cheaper), then existing blips
        if new_sequences.contains(&next_seq)
            || blip_exists(blips_table, topic_id, author, next_seq)?
        {
            watermark = Some(next_seq);
        } else {
            break;
        }
    }

    Ok(watermark)
}

/// Checks if a blip exists for the given topic:author:seq
fn blip_exists(
    table: &redb::Table<BlipsKey, &[u8]>,
    topic_id: &str,
    author: &str,
    seq_num: SequenceNumber,
) -> Result<bool, String> {
    let prefix = BlipsKeyPrefix::TopicAuthorSeq(topic_id.to_string(), author.to_string(), seq_num);

    // Use range query to check if any blip exists for this topic:author:seq
    let mut iter = table
        .range(prefix.range_start()..=prefix.range_end())
        .map_err(|e| format!("Failed to create iterator: {}", e))?;

    Ok(iter.next().is_some())
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[tokio::test(start_paused = true)]
    async fn store_records_blob_sources() {
        let dir = tempfile::tempdir().unwrap();
        let key = iroh::SecretKey::generate();
        let blob_sync = crate::BlobSync::new(key, dir.path().to_path_buf(), None)
            .await
            .unwrap();
        let source = iroh::SecretKey::from_bytes(&[7; 32]).public();
        let h = iroh_blobs::Hash::new([9; 32]);

        record_blob_sources(&blob_sync, &[h], source).await;

        let tried = HashSet::new();
        let (got, sources) = blob_sync
            .fetch_pool_for_test()
            .next_untried(&tried)
            .await
            .unwrap();
        assert_eq!(got, h);
        assert!(sources.contains(&source));
    }
}
