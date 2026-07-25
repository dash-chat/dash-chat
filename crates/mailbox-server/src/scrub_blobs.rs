use axum::{extract::State, http::StatusCode, Json};
use redb::{Database, ReadableDatabase, ReadableTable};
use serde::{Deserialize, Serialize};

use crate::{
    blob_refs_table::{
        blob_ref_key, key_is_for_blob, OpRef, BLOB_REFS_TABLE, REF_LIVE, REF_TOMBSTONED,
    },
    AppState, BlobSync,
};

/// One operation's reference to one blob.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct BlobRef {
    pub blob_hash: iroh_blobs::Hash,
    /// Opaque identifier of the referencing operation. Two messages carrying
    /// identical media are distinct references, so deleting one leaves the
    /// other's copy alone.
    pub op_ref: OpRef,
}

#[derive(Serialize, Deserialize)]
pub struct ScrubBlobsRequest {
    pub refs: Vec<BlobRef>,
}

#[derive(Serialize, Deserialize)]
pub struct ScrubBlobsResponse {
    /// Blobs whose last live reference this request tombstoned, and whose bytes
    /// the mailbox has therefore dropped.
    pub removed: Vec<iroh_blobs::Hash>,
}

/// Handle `POST /blobs/scrub`: tombstone the given `(blob, operation)`
/// references, then drop the bytes of any blob left with no live reference.
///
/// A tombstoned reference stays tombstoned permanently — `register_hashes`
/// refuses to re-record it — so a node that has not yet processed the delete
/// cannot resurrect the blob by re-announcing it.
pub async fn scrub_blobs(
    State(state): State<AppState>,
    Json(payload): Json<ScrubBlobsRequest>,
) -> Result<Json<ScrubBlobsResponse>, (StatusCode, String)> {
    let db = state.db.clone();
    let refs = payload.refs.clone();
    let orphaned = tokio::task::spawn_blocking(move || tombstone_refs(&db, &refs))
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

    for hash in &orphaned {
        drop_blob(&state.blob_sync, *hash).await;
    }

    tracing::debug!("Scrubbed {} blobs", orphaned.len());
    Ok(Json(ScrubBlobsResponse { removed: orphaned }))
}

/// Mark each reference tombstoned, returning the blobs left with no live
/// reference.
fn tombstone_refs(db: &Database, refs: &[BlobRef]) -> Result<Vec<iroh_blobs::Hash>, String> {
    let write_txn = db
        .begin_write()
        .map_err(|e| format!("Failed to begin transaction: {}", e))?;

    let mut orphaned = Vec::new();
    {
        let mut table = write_txn
            .open_table(BLOB_REFS_TABLE)
            .map_err(|e| format!("Failed to open blob refs table: {}", e))?;

        for blob_ref in refs {
            let key = blob_ref_key(&blob_ref.blob_hash, &blob_ref.op_ref);
            table
                .insert(key.as_slice(), REF_TOMBSTONED)
                .map_err(|e| format!("Failed to tombstone blob ref: {}", e))?;

            if !has_live_ref(&table, &blob_ref.blob_hash)?
                && !orphaned.contains(&blob_ref.blob_hash)
            {
                orphaned.push(blob_ref.blob_hash);
            }
        }
    }

    write_txn
        .commit()
        .map_err(|e| format!("Failed to commit transaction: {}", e))?;

    Ok(orphaned)
}

/// Whether any operation still holds a live reference to `blob_hash`.
fn has_live_ref(
    table: &redb::Table<&[u8], u8>,
    blob_hash: &iroh_blobs::Hash,
) -> Result<bool, String> {
    let start = blob_hash.as_bytes().to_vec();
    for entry in table
        .range(start.as_slice()..)
        .map_err(|e| format!("Failed to create iterator: {}", e))?
    {
        let (key, value) = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
        if !key_is_for_blob(key.value(), blob_hash) {
            break;
        }
        if value.value() == REF_LIVE {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Record a live reference from `op_ref` to each blob, skipping any reference
/// already tombstoned. Returns the hashes that are still wanted, i.e. those the
/// mailbox should go on to store or fetch.
///
/// An empty `op_ref` marks a *re-announce* rather than a new reference: it
/// records nothing and merely reports which blobs still have a live reference
/// from some operation. Clients re-announcing blobs the mailbox never fetched
/// have only a per-mailbox hash list to work from, no originating operation, so
/// letting them mint a reference would leave every re-announced blob holding an
/// unattributable one that no scrub could ever tombstone.
pub fn record_blob_refs(
    db: &Database,
    hashes: &[iroh_blobs::Hash],
    op_ref: &str,
) -> Result<Vec<iroh_blobs::Hash>, String> {
    let write_txn = db
        .begin_write()
        .map_err(|e| format!("Failed to begin transaction: {}", e))?;

    let mut wanted = Vec::new();
    {
        let mut table = write_txn
            .open_table(BLOB_REFS_TABLE)
            .map_err(|e| format!("Failed to open blob refs table: {}", e))?;

        for hash in hashes {
            if op_ref.is_empty() {
                if has_live_ref(&table, hash)? {
                    wanted.push(*hash);
                } else {
                    tracing::debug!(%hash, "ignoring re-announce of a blob with no live reference");
                }
                continue;
            }

            let key = blob_ref_key(hash, op_ref);
            let tombstoned = table
                .get(key.as_slice())
                .map_err(|e| format!("Failed to read blob ref: {}", e))?
                .is_some_and(|v| v.value() == REF_TOMBSTONED);
            if tombstoned {
                tracing::debug!(%hash, op_ref, "ignoring re-announce of a scrubbed blob reference");
                continue;
            }
            table
                .insert(key.as_slice(), REF_LIVE)
                .map_err(|e| format!("Failed to record blob ref: {}", e))?;
            wanted.push(*hash);
        }
    }

    write_txn
        .commit()
        .map_err(|e| format!("Failed to commit transaction: {}", e))?;

    Ok(wanted)
}

/// Whether the mailbox still wants `hash` at all, i.e. whether any live
/// reference to it remains. Used by the upload path, which carries no reference
/// of its own.
pub fn blob_is_wanted(db: &Database, hash: &iroh_blobs::Hash) -> Result<bool, String> {
    let read_txn = db
        .begin_read()
        .map_err(|e| format!("Failed to begin transaction: {}", e))?;
    let table = read_txn
        .open_table(BLOB_REFS_TABLE)
        .map_err(|e| format!("Failed to open blob refs table: {}", e))?;

    let start = hash.as_bytes().to_vec();
    let mut saw_ref = false;
    for entry in table
        .range(start.as_slice()..)
        .map_err(|e| format!("Failed to create iterator: {}", e))?
    {
        let (key, value) = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
        if !key_is_for_blob(key.value(), hash) {
            break;
        }
        saw_ref = true;
        if value.value() == REF_LIVE {
            return Ok(true);
        }
    }
    // A blob nobody has announced is not refused: uploads may legitimately
    // arrive before their announce. Only a blob whose every reference is
    // tombstoned is unwanted.
    Ok(!saw_ref)
}

/// Drop a blob the mailbox no longer has any live reference to: take it out of
/// the fetch pool (or it would be pulled back from a peer that still holds it)
/// and delete its retention tags so iroh's GC reclaims the bytes.
async fn drop_blob(blob_sync: &BlobSync, hash: iroh_blobs::Hash) {
    blob_sync.fetch_pool().remove(hash).await;
    blob_sync.release_blob(hash).await;
}
