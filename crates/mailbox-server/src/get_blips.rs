use axum::{extract::State, http::StatusCode, Json};
use redb::{Database, ReadableDatabase, ReadableTable};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

use crate::{
    AppState, Author, Blip, BlipsKey, BlipsKeyPrefix, LogHeights, LogId, SeqNum, TopicId,
    TopicLogKey, TopicLogs, WatermarksKey, BLIPS_TABLE, TOPIC_LOGS_TABLE, WATERMARKS_TABLE,
};

// Per-request, per-log cap on the missing-sequence range we materialize.
// `client_max_seq` comes straight from the (untrusted) request body, so without
// this a single cheap request forces an arbitrarily large server-side
// allocation. The client re-requests the next batch on its next sync, so capping
// is functionally transparent. Raise if logs legitimately outpace this per sync.
const MAX_MISSING_PER_LOG: SeqNum = 10_000;

#[derive(Serialize, Deserialize)]
pub struct GetBlipsRequest {
    /// The topics to resolve: every log the server associates with one of them
    /// is served.
    pub topics: BTreeSet<TopicId>,
    /// The highest sequence number the client holds per log. A served log that
    /// is absent here is sent in full; a log present here is also checked for
    /// entries the server is missing.
    #[serde(default)]
    pub heights: LogHeights,
}

#[derive(Serialize, Deserialize, Default)]
pub struct GetBlipsResponse {
    /// Which logs each requested topic resolves to.
    pub logs: TopicLogs,
    /// The blips that the client does not have.
    pub blips: BTreeMap<Author, BTreeMap<LogId, BTreeMap<SeqNum, Blip>>>,
    /// The blips that the server is missing from the client's heights.
    pub missing: BTreeMap<Author, BTreeMap<LogId, Vec<SeqNum>>>,
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
    let read_txn = db
        .begin_read()
        .map_err(|e| format!("Failed to begin transaction: {}", e))?;

    let blips_table = read_txn
        .open_table(BLIPS_TABLE)
        .map_err(|e| format!("Failed to open blips table: {}", e))?;

    let watermarks_table = read_txn
        .open_table(WATERMARKS_TABLE)
        .map_err(|e| format!("Failed to open watermarks table: {}", e))?;

    let topic_logs_table = read_txn
        .open_table(TOPIC_LOGS_TABLE)
        .map_err(|e| format!("Failed to open topic logs table: {}", e))?;

    let mut response = GetBlipsResponse::default();
    let mut served: BTreeSet<(Author, LogId)> = BTreeSet::new();

    for topic_id in &request.topics {
        let (start, end) = TopicLogKey::topic_range(topic_id);
        let mut logs: BTreeMap<Author, BTreeSet<LogId>> = BTreeMap::new();
        for entry in topic_logs_table
            .range(start..=end)
            .map_err(|e| format!("Failed to create iterator: {}", e))?
        {
            let (key, _) = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
            let key: TopicLogKey = key.value();
            served.insert((key.author.clone(), key.log_id.clone()));
            logs.entry(key.author).or_default().insert(key.log_id);
        }
        response.logs.insert(topic_id.clone(), logs);
    }

    for (author, log_id) in &served {
        let client_max_seq = request
            .heights
            .get(author)
            .and_then(|logs| logs.get(log_id))
            .copied();
        let stored = read_log(&blips_table, author, log_id)?;
        let new: BTreeMap<SeqNum, Blip> = stored
            .into_iter()
            // TODO: implement pagination or asynchronous data streaming
            // (https://www.ruststepbystep.com/how-to-stream-data-asynchronously-in-rust-with-axum/)
            // to handle huge amounts of blips being returned
            .filter(|(seq, _)| client_max_seq.is_none_or(|max| *seq > max))
            .collect();
        if !new.is_empty() {
            response
                .blips
                .entry(author.clone())
                .or_default()
                .insert(log_id.clone(), new);
        }
    }

    for (author, logs) in &request.heights {
        for (log_id, client_max_seq) in logs {
            let watermarks_key =
                WatermarksKey::new(author.clone(), log_id.clone()).map_err(|e| e.to_string())?;
            let server_watermark = watermarks_table
                .get(&watermarks_key)
                .map_err(|e| format!("Failed to read watermark: {}", e))?
                .map(|v| v.value());

            // Compute missing sequences:
            // - Everything 0..=watermark is NOT missing (we had it at some point)
            // - For sequences above watermark
            let candidates: Vec<SeqNum> = match server_watermark {
                // Server has contiguous sequences 0..=watermark
                Some(watermark) if *client_max_seq > watermark => {
                    let start = watermark.saturating_add(1);
                    let end = (*client_max_seq).min(start.saturating_add(MAX_MISSING_PER_LOG - 1));
                    (start..=end).collect()
                }
                // client_max_seq <= watermark: server has everything
                Some(_) => Vec::new(),
                None => {
                    // No watermark = no contiguous sequences from 0
                    let end = (*client_max_seq).min(MAX_MISSING_PER_LOG - 1);
                    (0..=end).collect()
                }
            };
            if candidates.is_empty() {
                continue;
            }

            // Only report sequences we don't have stored at all.
            let stored = read_log(&blips_table, author, log_id)?;
            let missing_seq_nums: Vec<SeqNum> = candidates
                .into_iter()
                .filter(|seq| !stored.contains_key(seq))
                .collect();

            if !missing_seq_nums.is_empty() {
                tracing::debug!(
                    "Server missing {} blips for {}:{} (sequences: {:?})",
                    missing_seq_nums.len(),
                    author,
                    log_id,
                    missing_seq_nums
                );
                response
                    .missing
                    .entry(author.clone())
                    .or_default()
                    .insert(log_id.clone(), missing_seq_nums);
            }
        }
    }

    tracing::debug!("Retrieved blips for {} topics", request.topics.len());
    Ok(response)
}

/// Every stored entry of one author's log, by sequence number.
fn read_log(
    blips_table: &impl ReadableTable<BlipsKey, &'static [u8]>,
    author: &str,
    log_id: &str,
) -> Result<BTreeMap<SeqNum, Blip>, String> {
    let prefix = BlipsKeyPrefix::AuthorLog(author.to_string(), log_id.to_string());
    let mut log = BTreeMap::new();
    for entry in blips_table
        .range(prefix.range_start()..=prefix.range_end())
        .map_err(|e| format!("Failed to create iterator: {}", e))?
    {
        let (key, value) = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
        let key: BlipsKey = key.value();
        log.insert(key.sequence_number, Blip::from(value.value().to_vec()));
    }
    Ok(log)
}
