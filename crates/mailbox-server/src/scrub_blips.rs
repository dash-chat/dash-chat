use axum::{extract::State, http::StatusCode, Json};
use redb::{Database, ReadableTable};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::{
    scrub_table::{decode_commitment, ScrubHash, SCRUB_TABLE},
    AppState, Author, Blip, BlipsKey, BlipsKeyPrefix, SequenceNumber, TopicId, BLIPS_TABLE,
};

/// A request to replace stored blips with their payload-free forms.
///
/// The submitted bytes are the replacement itself: the mailbox accepts them
/// only where they hash to the commitment the blip's publisher supplied at
/// store time.
#[derive(Serialize, Deserialize)]
pub struct ScrubBlipsRequest {
    pub blips: BTreeMap<TopicId, BTreeMap<Author, BTreeMap<SequenceNumber, Blip>>>,
}

/// A single blip coordinate, as reported back in the response.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScrubbedBlip {
    pub topic_id: TopicId,
    pub author: Author,
    pub sequence_number: SequenceNumber,
}

#[derive(Serialize, Deserialize)]
pub struct ScrubBlipsResponse {
    /// Coordinates whose stored blip is now payload-free.
    pub scrubbed: Vec<ScrubbedBlip>,
    /// Coordinates left untouched: no blip stored there, no commitment recorded
    /// for it, or the submitted bytes did not match the commitment.
    pub rejected: Vec<ScrubbedBlip>,
}

pub async fn scrub_blips(
    State(state): State<AppState>,
    Json(payload): Json<ScrubBlipsRequest>,
) -> Result<Json<ScrubBlipsResponse>, (StatusCode, String)> {
    let db = state.db.clone();
    // spawn_blocking for the same reason as store_blips: redb's begin_write()
    // blocks waiting for exclusive write access.
    tokio::task::spawn_blocking(move || scrub_blips_inner(&db, &payload))
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

fn scrub_blips_inner(
    db: &Database,
    request: &ScrubBlipsRequest,
) -> Result<ScrubBlipsResponse, String> {
    let write_txn = db
        .begin_write()
        .map_err(|e| format!("Failed to begin transaction: {}", e))?;

    let mut scrubbed = Vec::new();
    let mut rejected = Vec::new();

    {
        let mut blips_table = write_txn
            .open_table(BLIPS_TABLE)
            .map_err(|e| format!("Failed to open blips table: {}", e))?;
        let scrub_table = write_txn
            .open_table(SCRUB_TABLE)
            .map_err(|e| format!("Failed to open scrub table: {}", e))?;

        for (topic_id, authors) in &request.blips {
            for (author, sequences) in authors {
                for (seq_num, submitted) in sequences {
                    let coordinate = ScrubbedBlip {
                        topic_id: topic_id.clone(),
                        author: author.clone(),
                        sequence_number: *seq_num,
                    };
                    let keys = matching_keys(
                        &blips_table,
                        &scrub_table,
                        topic_id,
                        author,
                        *seq_num,
                        submitted,
                    )?;
                    if keys.is_empty() {
                        rejected.push(coordinate);
                        continue;
                    }
                    for key in keys {
                        blips_table
                            .insert(&key, submitted.as_slice())
                            .map_err(|e| format!("Failed to scrub blip: {}", e))?;
                    }
                    scrubbed.push(coordinate);
                }
            }
        }
    }

    write_txn
        .commit()
        .map_err(|e| format!("Failed to commit transaction: {}", e))?;

    tracing::debug!(
        "Scrubbed {} blips ({} rejected)",
        scrubbed.len(),
        rejected.len()
    );
    Ok(ScrubBlipsResponse { scrubbed, rejected })
}

/// The stored rows at `topic:author:seq` whose commitment the submitted bytes
/// satisfy. A coordinate can hold more than one row — `store_blips` inserts
/// under a fresh UUID each time — and each row carries its own commitment.
fn matching_keys(
    blips_table: &redb::Table<BlipsKey, &[u8]>,
    scrub_table: &redb::Table<BlipsKey, &[u8]>,
    topic_id: &str,
    author: &str,
    seq_num: SequenceNumber,
    submitted: &Blip,
) -> Result<Vec<BlipsKey>, String> {
    let submitted_hash = ScrubHash::new(submitted.as_slice());
    let prefix = BlipsKeyPrefix::TopicAuthorSeq(topic_id.to_string(), author.to_string(), seq_num);

    let mut keys = Vec::new();
    for entry in blips_table
        .range(prefix.range_start()..=prefix.range_end())
        .map_err(|e| format!("Failed to create iterator: {}", e))?
    {
        let (key, _) = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
        let key = key.value();
        let commitment = scrub_table
            .get(&key)
            .map_err(|e| format!("Failed to read commitment: {}", e))?
            .and_then(|v| decode_commitment(v.value()));
        if commitment == Some(submitted_hash) {
            keys.push(key);
        }
    }
    Ok(keys)
}
