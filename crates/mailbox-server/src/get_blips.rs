use axum::{extract::State, http::StatusCode, Json};
use redb::{Database, ReadableDatabase};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

use crate::{
    AppState, Author, Blip, BlipsKey, BlipsKeyPrefix, SequenceNumber, TopicId, WatermarksKey,
    BLIPS_TABLE, WATERMARKS_TABLE,
};

#[derive(Serialize, Deserialize)]
pub struct GetBlipsRequest {
    pub topics: BTreeMap<TopicId, BTreeMap<Author, SequenceNumber>>,
}

#[derive(Serialize, Deserialize)]
pub struct GetBlipsForTopicResponse {
    // The blips that the client does not have
    pub blips: BTreeMap<Author, BTreeMap<SequenceNumber, Blip>>,
    // The blips that the server is missing from the client's request
    pub missing: BTreeMap<Author, Vec<SequenceNumber>>,
}

#[derive(Serialize, Deserialize)]
pub struct GetBlipsResponse {
    pub blips_by_topic: BTreeMap<TopicId, GetBlipsForTopicResponse>,
}

pub async fn get_blips_for_topics(
    State(state): State<AppState>,
    Json(payload): Json<GetBlipsRequest>,
) -> Result<Json<GetBlipsResponse>, (StatusCode, String)> {
    let db = state.db.clone();
    // Use spawn_blocking because redb's begin_read() can block while waiting for
    // concurrent write transactions. Running this directly in async context would
    // block tokio worker threads and cause deadlocks under concurrent load.
    tokio::task::spawn_blocking(move || get_blips_for_topics_inner(&db, &payload))
        .await
        .map_err(|e| {
            tracing::error!("Task join error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal server error".to_string(),
            )
        })?
        .map(Json)
        .map_err(|e| {
            tracing::error!("{}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal server error".to_string(),
            )
        })
}

fn get_blips_for_topics_inner(
    db: &Database,
    request: &GetBlipsRequest,
) -> Result<GetBlipsResponse, String> {
    let mut blips_by_topic: BTreeMap<TopicId, GetBlipsForTopicResponse> = BTreeMap::new();

    let read_txn = db
        .begin_read()
        .map_err(|e| format!("Failed to begin transaction: {}", e))?;

    let blips_table = read_txn
        .open_table(BLIPS_TABLE)
        .map_err(|e| format!("Failed to open blips table: {}", e))?;

    let watermarks_table = read_txn
        .open_table(WATERMARKS_TABLE)
        .map_err(|e| format!("Failed to open watermarks table: {}", e))?;

    for (topic_id, requested_authors) in &request.topics {
        let mut topic_authors: BTreeMap<Author, BTreeMap<SequenceNumber, Blip>> = BTreeMap::new();
        // Track which sequences we have stored for each requested author
        // (used to avoid reporting as missing sequences we actually have)
        let mut stored_seqs_per_author: BTreeMap<Author, BTreeSet<SequenceNumber>> =
            BTreeMap::new();

        // Use prefix-based range query to only iterate over blips for this topic
        let prefix = BlipsKeyPrefix::Topic(topic_id.clone());

        for entry in blips_table
            .range(prefix.range_start()..=prefix.range_end())
            .map_err(|e| format!("Failed to create iterator: {}", e))?
        {
            let (key, value) = entry.map_err(|e| format!("Failed to read entry: {}", e))?;

            let blip_key: BlipsKey = key.value();
            let author = blip_key.author.clone();
            let seq_num = blip_key.sequence_number;

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
                // Author is NOT in the request: include all blips for this author
                // TODO: implement pagination or asynchronous data streaming
                // (https://www.ruststepbystep.com/how-to-stream-data-asynchronously-in-rust-with-axum/)
                // to handle huge amounts of blips being returned
                true
            };

            if should_include {
                topic_authors
                    .entry(author)
                    .or_insert_with(BTreeMap::new)
                    .insert(seq_num, Blip::from(value.value().to_vec()));
            }
        }

        // Calculate missing blips using watermarks and stored sequences
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
                    "Server missing {} blips for author {} in topic {} (sequences: {:?})",
                    missing_seq_nums.len(),
                    author,
                    topic_id,
                    missing_seq_nums
                );
                missing.insert(author.clone(), missing_seq_nums);
            }
        }

        blips_by_topic.insert(
            topic_id.clone(),
            GetBlipsForTopicResponse {
                blips: topic_authors,
                missing,
            },
        );
    }

    tracing::debug!("Retrieved blips for {} topics", request.topics.len());
    Ok(GetBlipsResponse { blips_by_topic })
}
