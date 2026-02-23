use axum::{extract::State, http::StatusCode, Json};
use redb::{Database, ReadableDatabase};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

use crate::{
    AppState, Author, Blob, BlobsKey, BlobsKeyPrefix, SequenceNumber, TopicId, WatermarksKey,
    BLOBS_TABLE, WATERMARKS_TABLE,
};

#[derive(Serialize, Deserialize)]
pub struct GetBlobsRequest {
    pub topics: BTreeMap<TopicId, BTreeMap<Author, SequenceNumber>>,
}

#[derive(Serialize, Deserialize)]
pub struct GetBlobsForTopicResponse {
    // The blobs that the client does not have
    pub blobs: BTreeMap<Author, BTreeMap<SequenceNumber, Blob>>,
    // The blobs that the server is missing from the client's request
    pub missing: BTreeMap<Author, Vec<SequenceNumber>>,
}

#[derive(Serialize, Deserialize)]
pub struct GetBlobsResponse {
    pub blobs_by_topic: BTreeMap<TopicId, GetBlobsForTopicResponse>,
}

pub async fn get_blobs_for_topics(
    State(state): State<AppState>,
    Json(payload): Json<GetBlobsRequest>,
) -> Result<Json<GetBlobsResponse>, (StatusCode, String)> {
    let db = state.db.clone();
    // Use spawn_blocking because redb's begin_read() can block while waiting for
    // concurrent write transactions. Running this directly in async context would
    // block tokio worker threads and cause deadlocks under concurrent load.
    tokio::task::spawn_blocking(move || get_blobs_for_topics_inner(&db, &payload))
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

fn get_blobs_for_topics_inner(
    db: &Database,
    request: &GetBlobsRequest,
) -> Result<GetBlobsResponse, String> {
    let mut blobs_by_topic: BTreeMap<TopicId, GetBlobsForTopicResponse> = BTreeMap::new();

    let read_txn = db
        .begin_read()
        .map_err(|e| format!("Failed to begin transaction: {}", e))?;

    let blobs_table = read_txn
        .open_table(BLOBS_TABLE)
        .map_err(|e| format!("Failed to open blobs table: {}", e))?;

    let watermarks_table = read_txn
        .open_table(WATERMARKS_TABLE)
        .map_err(|e| format!("Failed to open watermarks table: {}", e))?;

    for (topic_id, requested_authors) in &request.topics {
        let mut topic_authors: BTreeMap<Author, BTreeMap<SequenceNumber, Blob>> = BTreeMap::new();
        // Track which sequences we have stored for each requested author
        // (used to avoid reporting as missing sequences we actually have)
        let mut stored_seqs_per_author: BTreeMap<Author, BTreeSet<SequenceNumber>> =
            BTreeMap::new();

        // Use prefix-based range query to only iterate over blobs for this topic
        let prefix = BlobsKeyPrefix::Topic(topic_id.clone());

        for entry in blobs_table
            .range(prefix.range_start()..=prefix.range_end())
            .map_err(|e| format!("Failed to create iterator: {}", e))?
        {
            let (key, value) = entry.map_err(|e| format!("Failed to read entry: {}", e))?;

            let blob_key: BlobsKey = key.value();
            let author = blob_key.author.clone();
            let seq_num = blob_key.sequence_number;

            // Track sequences we have for requested authors (for missing calculation)
            if requested_authors.contains_key(&author) {
                stored_seqs_per_author
                    .entry(author.clone())
                    .or_default()
                    .insert(seq_num);
            }

            // Check if this author was requested with a specific sequence number filter
            let should_include = if let Some(min_seq_num) = requested_authors.get(&author) {
                // Author is in the request: only include if seq_num > min_seq_num
                seq_num > *min_seq_num
            } else {
                // Author is NOT in the request: include all blobs for this author
                // TODO: implement pagination or asynchronous data streaming
                // (https://www.ruststepbystep.com/how-to-stream-data-asynchronously-in-rust-with-axum/)
                // to handle huge amounts of blobs being returned
                true
            };

            if should_include {
                topic_authors
                    .entry(author)
                    .or_insert_with(BTreeMap::new)
                    .insert(seq_num, Blob::from(value.value().to_vec()));
            }
        }

        // Calculate missing blobs using watermarks and stored sequences
        let mut missing: BTreeMap<Author, Vec<SequenceNumber>> = BTreeMap::new();
        for (author, client_max_seq) in requested_authors {
            let watermarks_key =
                WatermarksKey::new(topic_id.clone(), author.clone()).map_err(|e| e.to_string())?;

            // Get watermark for this topic:author
            let server_watermark = watermarks_table
                .get(&watermarks_key)
                .map_err(|e| format!("Failed to read watermark: {}", e))?
                .map(|v| v.value());

            // Get sequences we have stored for this author
            let empty = BTreeSet::new();
            let stored_seqs = stored_seqs_per_author.get(author).unwrap_or(&empty);

            // Compute missing sequences:
            // - Everything 0..=watermark is NOT missing (we had it at some point)
            // - For sequences above watermark
            let missing_seq_nums: Vec<SequenceNumber> = match server_watermark {
                Some(watermark) => {
                    // Server has contiguous sequences 0..=watermark
                    if *client_max_seq > watermark {
                        ((watermark + 1)..=*client_max_seq).collect()
                    } else {
                        // client_max_seq <= watermark: server has everything
                        Vec::new()
                    }
                }
                None => {
                    // No watermark = no contiguous sequences from 0
                    (0..=*client_max_seq).collect()
                }
            };

            // Only include in missing if we don't have this sequence stored
            let missing_seq_nums: Vec<SequenceNumber> = missing_seq_nums
                .into_iter()
                .filter(|seq| !stored_seqs.contains(seq))
                .collect();

            if !missing_seq_nums.is_empty() {
                tracing::debug!(
                    "Server missing {} blobs for author {} in topic {} (sequences: {:?})",
                    missing_seq_nums.len(),
                    author,
                    topic_id,
                    missing_seq_nums
                );
                missing.insert(author.clone(), missing_seq_nums);
            }
        }

        blobs_by_topic.insert(
            topic_id.clone(),
            GetBlobsForTopicResponse {
                blobs: topic_authors,
                missing,
            },
        );
    }

    tracing::debug!("Retrieved blobs for {} topics", request.topics.len());
    Ok(GetBlobsResponse { blobs_by_topic })
}
