use super::*;
use redb::ReadableDatabase;
use redb::ReadableTable;
use std::collections::HashSet;
use std::path::Path;

/// Table mapping `u64` queue ID → `[32 bytes hash | utf-8 mailbox_id]`.
/// The u64 key provides FIFO ordering.
const BLOB_QUEUE_TABLE: redb::TableDefinition<u64, &[u8]> =
    redb::TableDefinition::new("blob_publish_queue");

/// Tracks the next ID to assign. Stored as a single-row table so it
/// survives restarts without scanning the queue table.
const BLOB_QUEUE_SEQ_TABLE: redb::TableDefinition<(), u64> =
    redb::TableDefinition::new("blob_publish_queue_seq");

fn encode_value(blob_hash: &OpaqHash, mailbox_id: &str) -> Vec<u8> {
    let hash_bytes: [u8; 32] = *blob_hash.as_bytes();
    let mut buf = Vec::with_capacity(32 + mailbox_id.len());
    buf.extend_from_slice(&hash_bytes);
    buf.extend_from_slice(mailbox_id.as_bytes());
    buf
}

fn decode_value(bytes: &[u8]) -> anyhow::Result<(OpaqHash, MailboxId)> {
    anyhow::ensure!(bytes.len() >= 32, "blob queue value too short");
    let hash = OpaqHash::from_bytes(bytes[..32].try_into().unwrap());
    let mailbox_id = std::str::from_utf8(&bytes[32..])?.to_string();
    Ok((hash, mailbox_id))
}

/// A redb-backed [`BlobPublishQueue`] that persists entries to disk.
///
/// In-flight tracking is kept in memory: on restart, all entries are
/// treated as pending (safe because publish is idempotent on the server side).
#[derive(Clone)]
pub struct RedbBlobPublishQueue {
    db: Arc<redb::Database>,
    in_flight: Arc<Mutex<HashSet<u64>>>,
}

impl RedbBlobPublishQueue {
    /// Open or create the queue database at the given path.
    pub fn new(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let db = redb::Database::create(path)?;
        // Ensure tables exist.
        let txn = db.begin_write()?;
        {
            let _ = txn.open_table(BLOB_QUEUE_TABLE)?;
            let _ = txn.open_table(BLOB_QUEUE_SEQ_TABLE)?;
        }
        txn.commit()?;
        Ok(Self {
            db: Arc::new(db),
            in_flight: Arc::new(Mutex::new(HashSet::new())),
        })
    }

    /// Open the queue using an existing shared `Database` handle
    /// (e.g. the same redb instance used by `LocalStore`).
    pub fn from_db(db: Arc<redb::Database>) -> anyhow::Result<Self> {
        let txn = db.begin_write()?;
        {
            let _ = txn.open_table(BLOB_QUEUE_TABLE)?;
            let _ = txn.open_table(BLOB_QUEUE_SEQ_TABLE)?;
        }
        txn.commit()?;
        Ok(Self {
            db,
            in_flight: Arc::new(Mutex::new(HashSet::new())),
        })
    }
}

#[async_trait::async_trait]
impl BlobPublishQueue for RedbBlobPublishQueue {
    async fn enqueue(&self, blob_hash: OpaqHash, mailbox_id: MailboxId) -> anyhow::Result<()> {
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            // Idempotency check: scan for existing (hash, mailbox_id) pair.
            let txn = db.begin_read()?;
            {
                let table = txn.open_table(BLOB_QUEUE_TABLE)?;
                for entry in table.iter()? {
                    let (_k, v): (redb::AccessGuard<'_, u64>, redb::AccessGuard<'_, &[u8]>) =
                        entry?;
                    let (h, m) = decode_value(v.value())?;
                    if h == blob_hash && m == mailbox_id {
                        return Ok(());
                    }
                }
            }
            drop(txn);

            // Allocate ID and insert.
            let write_txn = db.begin_write()?;
            let id;
            {
                let mut seq = write_txn.open_table(BLOB_QUEUE_SEQ_TABLE)?;
                let current = seq.get(())?.map(|v| v.value()).unwrap_or(0);
                id = current + 1;
                seq.insert((), id)?;
            }
            {
                let mut table = write_txn.open_table(BLOB_QUEUE_TABLE)?;
                table.insert(id, encode_value(&blob_hash, &mailbox_id).as_slice())?;
            }
            write_txn.commit()?;
            Ok(())
        })
        .await?
    }

    async fn dequeue_batch(&self, limit: usize) -> anyhow::Result<Vec<BlobPublishEntry>> {
        let db = self.db.clone();
        let in_flight = self.in_flight.clone();
        tokio::task::spawn_blocking(move || {
            let txn = db.begin_read()?;
            let table = txn.open_table(BLOB_QUEUE_TABLE)?;
            let mut in_flight = in_flight.blocking_lock();
            let mut batch = Vec::new();
            let mut items = 0;
            for entry in table.iter()? {
                let (k, v): (redb::AccessGuard<'_, u64>, redb::AccessGuard<'_, &[u8]>) = entry?;
                let id = k.value();
                if in_flight.contains(&id) {
                    continue;
                }
                let (blob_hash, mailbox_id) = decode_value(v.value())?;
                in_flight.insert(id);
                batch.push(BlobPublishEntry {
                    id,
                    blob_hash,
                    mailbox_id,
                });
                items += 1;
                if batch.len() >= limit {
                    break;
                }
            }
            dbg!(&items);
            Ok(batch)
        })
        .await?
    }

    async fn ack(&self, id: u64) -> anyhow::Result<()> {
        let db = self.db.clone();
        let in_flight = self.in_flight.clone();
        tokio::task::spawn_blocking(move || {
            let txn = db.begin_write()?;
            {
                let mut table = txn.open_table(BLOB_QUEUE_TABLE)?;
                table.remove(id)?;
            }
            txn.commit()?;
            in_flight.blocking_lock().remove(&id);
            Ok(())
        })
        .await?
    }

    async fn nack(&self, id: u64) -> anyhow::Result<()> {
        self.in_flight.lock().await.remove(&id);
        Ok(())
    }

    async fn pending_count(&self) -> anyhow::Result<usize> {
        let db = self.db.clone();
        let in_flight = self.in_flight.clone();
        tokio::task::spawn_blocking(move || {
            let txn = db.begin_read()?;
            let table = txn.open_table(BLOB_QUEUE_TABLE)?;
            let in_flight = in_flight.blocking_lock();
            let count = table
                .iter()?
                .filter_map(
                    |e: Result<(redb::AccessGuard<'_, u64>, redb::AccessGuard<'_, &[u8]>), _>| {
                        e.ok()
                    },
                )
                .filter(|(k, _)| !in_flight.contains(&k.value()))
                .count();
            Ok(count)
        })
        .await?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_redb_queue_basic_flow() {
        let dir = tempfile::tempdir().unwrap();
        let q = RedbBlobPublishQueue::new(dir.path().join("queue.redb")).unwrap();

        let hash = OpaqHash::from_bytes([1u8; 32]);
        let mailbox = "mailbox-1".to_string();

        // Enqueue
        q.enqueue(hash.clone(), mailbox.clone()).await.unwrap();
        assert_eq!(q.pending_count().await.unwrap(), 1);

        // Idempotent enqueue
        q.enqueue(hash.clone(), mailbox.clone()).await.unwrap();
        assert_eq!(q.pending_count().await.unwrap(), 1);

        // Different mailbox = different entry
        q.enqueue(hash.clone(), "mailbox-2".to_string())
            .await
            .unwrap();
        assert_eq!(q.pending_count().await.unwrap(), 2);

        // Dequeue
        let batch = q.dequeue_batch(10).await.unwrap();
        assert_eq!(batch.len(), 2);
        assert_eq!(q.pending_count().await.unwrap(), 0); // all in-flight

        // Ack first, nack second
        q.ack(batch[0].id).await.unwrap();
        q.nack(batch[1].id).await.unwrap();
        assert_eq!(q.pending_count().await.unwrap(), 1); // nacked one is back

        // Dequeue again gets only the nacked entry
        let batch2 = q.dequeue_batch(10).await.unwrap();
        assert_eq!(batch2.len(), 1);
        assert_eq!(batch2[0].id, batch[1].id);
    }

    #[tokio::test]
    async fn test_redb_queue_survives_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("queue.redb");

        let hash = OpaqHash::from_bytes([42u8; 32]);
        let mailbox = "srv".to_string();

        {
            let q = RedbBlobPublishQueue::new(&path).unwrap();
            q.enqueue(hash.clone(), mailbox.clone()).await.unwrap();
            assert_eq!(q.pending_count().await.unwrap(), 1);
        }

        // Reopen — entry should still be there, in-flight state reset
        let q = RedbBlobPublishQueue::new(&path).unwrap();
        assert_eq!(q.pending_count().await.unwrap(), 1);
        let batch = q.dequeue_batch(10).await.unwrap();
        assert_eq!(batch.len(), 1);
        assert_eq!(batch[0].blob_hash, hash);
        assert_eq!(batch[0].mailbox_id, mailbox);
    }

    #[tokio::test]
    async fn test_redb_queue_fifo_order() {
        let dir = tempfile::tempdir().unwrap();
        let q = RedbBlobPublishQueue::new(dir.path().join("queue.redb")).unwrap();

        for i in 0..5u8 {
            q.enqueue(OpaqHash::from_bytes([i; 32]), "m".to_string())
                .await
                .unwrap();
        }

        let batch = q.dequeue_batch(5).await.unwrap();
        for (i, entry) in batch.iter().enumerate() {
            assert_eq!(entry.blob_hash, OpaqHash::from_bytes([i as u8; 32]));
        }
    }
}
