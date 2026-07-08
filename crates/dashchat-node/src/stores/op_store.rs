pub mod queries;

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::{Arc, RwLock},
};

use p2panda::Hash;
#[cfg(any(test, feature = "testing"))]
use p2panda::operation::Header;
use p2panda::operation::{LogId, Operation};
use p2panda_core::SeqNum;
use p2panda_store::SqliteStore;
use p2panda_store::logs::LogStore;

use crate::{mailbox::MailboxOperation, topic::TopicId, util::first, *};

#[derive(Clone, derive_more::Deref, derive_more::DerefMut)]
pub struct OpStore {
    #[deref]
    #[deref_mut]
    pub(crate) store: SqliteStore,

    #[cfg(feature = "testing")]
    pub processed_ops: Arc<RwLock<HashMap<TopicId, HashSet<Hash>>>>,
}

impl OpStore {
    pub async fn new(database_file_path: impl AsRef<std::path::Path>) -> anyhow::Result<Self> {
        let path = database_file_path.as_ref().to_path_buf();
        let url = format!("sqlite://{}", path.to_string_lossy());
        p2panda_store::sqlite::create_database(&url).await?;

        let pool = sqlx::SqlitePool::connect(&url)
            .await
            .map_err(|e| anyhow::anyhow!("failed to connect to sqlite at '{path:?}': {e}"))?;

        if p2panda_store::sqlite::run_pending_migrations(&pool)
            .await
            .is_err()
        {
            pool.close().await;
            panic!("Database migration failed");
        }
        let store = SqliteStore::from_pool(pool);
        let store = Self {
            store,
            processed_ops: Arc::new(RwLock::new(HashMap::new())),
        };
        store.init_processed_ops().await?;
        Ok(store)
    }

    pub fn from_sqlite(store: SqliteStore) -> Self {
        Self {
            store,
            processed_ops: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn temporary_sqlite() -> anyhow::Result<Self> {
        let store = SqliteStore::temporary().await;
        let store = Self::from_sqlite(store);
        store.init_processed_ops().await?;
        Ok(store)
    }

    /// Create the processed-ops table if it doesn't exist yet. On first
    /// creation, backfill it with every already-stored operation: those
    /// predate the table and have long been processed (and transmitted).
    ///
    /// The table records which operations have completed application-layer
    /// processing. Mailbox sync only transmits the contiguous processed prefix
    /// of each log, so an operation whose payload might still be tombstoned by
    /// pending processing is never sent onward.
    pub async fn init_processed_ops(&self) -> anyhow::Result<()> {
        let pool = self.store.pool();
        let existed: Option<(i64,)> = sqlx::query_as(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'dashchat_processed_ops_v1'",
        )
        .fetch_optional(pool)
        .await?;
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS dashchat_processed_ops_v1 (hash TEXT NOT NULL PRIMARY KEY)",
        )
        .execute(pool)
        .await?;
        if existed.is_none() {
            sqlx::query(
                "INSERT OR IGNORE INTO dashchat_processed_ops_v1 (hash) SELECT hash FROM operations_v1",
            )
            .execute(pool)
            .await?;
        }
        Ok(())
    }

    /// Record that an operation has completed application-layer processing,
    /// making it eligible for mailbox transmission.
    pub async fn mark_op_processed_persistent(&self, hash: &Hash) -> anyhow::Result<()> {
        sqlx::query("INSERT OR IGNORE INTO dashchat_processed_ops_v1 (hash) VALUES (?)")
            .bind(hash.to_string())
            .execute(self.store.pool())
            .await?;
        Ok(())
    }

    async fn is_op_processed_persistent(&self, hash: &Hash) -> anyhow::Result<bool> {
        let row: Option<(i64,)> =
            sqlx::query_as("SELECT 1 FROM dashchat_processed_ops_v1 WHERE hash = ?")
                .bind(hash.to_string())
                .fetch_optional(self.store.pool())
                .await?;
        Ok(row.is_some())
    }

    /// Gracefully close the underlying SQLite pool (no-op for the in-memory variant).
    pub async fn close(&self) {
        self.store.pool().close().await;
    }

    pub async fn get_log(
        &self,
        author: &DeviceId,
        log_id: &LogId,
        from: Option<u64>,
    ) -> anyhow::Result<Vec<Operation>> {
        let log = self
            .store
            .get_log_entries(author, log_id, from, None)
            .await?
            .unwrap_or_else(|| {
                tracing::warn!(
                    "No log found for log_id {} and author {}",
                    Hash::from_bytes(*log_id.as_bytes()),
                    author
                );
                vec![]
            })
            .into_iter()
            .map(first)
            .collect();
        Ok(log)
    }

    pub async fn get_operation(&self, hash: &Hash) -> anyhow::Result<Option<Operation>> {
        use p2panda_store::operations::OperationStore;
        OperationStore::<Operation, Hash, LogId>::get_operation(&self.store, hash)
            .await
            .map_err(|err| anyhow::anyhow!("failed to get operation for {hash:?}: {err}"))
    }

    #[deprecated = "will be replace by proper use of p2panda-streams"]
    pub fn get_all_operations_not_fully_sorted(
        &self,
    ) -> impl futures::Stream<Item = Result<Operation, anyhow::Error>> + '_ {
        queries::get_all_operations_not_fully_sorted(&self.store)
    }

    /// Get the "height" of each log, which is actually the highest sequence number of the log.
    pub async fn get_log_heights(
        &self,
        log_id: &LogId,
    ) -> Result<BTreeMap<DeviceId, SeqNum>, anyhow::Error> {
        let log_id: LogId = log_id.to_owned().into();
        queries::get_log_heights_by_author(&self.store, &log_id).await
    }

    /// Get the interleaved logs for a topic and a list of authors.
    ///
    /// This is only used for testing and should stay that way.
    #[cfg(any(test, feature = "testing"))]
    pub async fn get_interleaved_logs(
        &self,
        log_id: LogId,
        authors: Vec<DeviceId>,
    ) -> anyhow::Result<Vec<(Header, Option<Payload>)>> {
        let mut logs = Vec::new();
        for author in authors {
            for op in self.get_log(&author, &log_id, None).await? {
                if let Some(body) = op.body {
                    if let Ok(payload) = Payload::try_from_body(&body) {
                        logs.push((op.header, Some(payload)));
                    } else {
                        tracing::error!("Failed to decode payload: {body:?}");
                    }
                } else {
                    logs.push((op.header, None));
                }
            }
        }
        logs.sort_by_key(|(h, _)| h.timestamp);
        Ok(logs)
    }

    /// Drop the stored payload (body) of an operation, leaving its header
    /// intact so log sync stays consistent. Used to enforce tombstones.
    pub async fn delete_body(&self, hash: &Hash) -> anyhow::Result<()> {
        use p2panda_store::operations::OperationStore;
        OperationStore::<Operation, Hash, LogId>::delete_operation_payload(&self.store, hash)
            .await?;
        Ok(())
    }

    pub async fn get_authors(&self, log_id: LogId) -> anyhow::Result<HashSet<DeviceId>> {
        let authors = self
            .get_log_heights(&log_id)
            .await?
            .keys()
            .cloned()
            .collect::<HashSet<_>>();
        Ok(authors)
    }

    #[cfg(feature = "testing")]
    pub fn mark_op_processed(&self, topic: TopicId, hash: &Hash) {
        self.processed_ops
            .write()
            .unwrap()
            .entry(topic)
            .or_default()
            .insert(hash.clone());
    }

    #[cfg(feature = "testing")]
    pub fn is_op_processed(&self, topic: &TopicId, hash: &Hash) -> bool {
        self.processed_ops
            .read()
            .unwrap()
            .get(topic)
            .map(|s| s.contains(hash))
            .unwrap_or(false)
    }
}

#[async_trait::async_trait]
impl mailbox_client::store::MailboxStore<MailboxOperation> for OpStore {
    async fn get_log(
        &self,
        author: &DeviceId,
        topic: &TopicId,
        from: u64,
    ) -> Result<Option<Vec<MailboxOperation>>, anyhow::Error> {
        let log_id = LogId::from_topic(*topic);
        let from = if from == 0 { None } else { Some(from - 1) };
        let log = self
            .store
            .get_log_entries(author, &log_id, from, None)
            .await
            .map_err(|err| {
                anyhow::anyhow!("failed to get log for {author:?}: {log_id:?}: {err}")
            })?;

        let Some(log) = log else {
            return Ok(None);
        };

        // Only transmit the contiguous prefix of fully-processed operations.
        // An operation that hasn't completed application-layer processing may
        // still have its payload dropped by a tombstone it is about to
        // enforce, so it must not be sent onward yet. Truncating (rather than
        // filtering) keeps the returned log dense from `from`, which callers
        // index by sequence number.
        let mut ops = Vec::with_capacity(log.len());
        for (op, _) in log {
            if !self.is_op_processed_persistent(&op.hash).await? {
                break;
            }
            ops.push(MailboxOperation {
                topic: *topic,
                header: op.header,
                body: op.body,
            });
        }
        Ok(Some(ops))
    }

    async fn get_log_heights(&self, topic: &TopicId) -> anyhow::Result<Vec<(DeviceId, u64)>> {
        Ok(OpStore::get_log_heights(self, &LogId::from_topic(*topic))
            .await?
            .into_iter()
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use p2panda::operation::{Extensions, Header};
    use p2panda_core::{Body, PruneFlag, Timestamp};
    use p2panda_store::Transaction;
    use p2panda_store::operations::OperationStore;

    use super::*;

    async fn fetch(store: &OpStore, hash: &Hash) -> Operation {
        OperationStore::<Operation, Hash, LogId>::get_operation(&store.store, hash)
            .await
            .unwrap()
            .unwrap()
    }

    fn signed_op(
        signing_key: &p2panda::SigningKey,
        log_id: LogId,
        seq_num: u64,
        backlink: Option<Hash>,
        payload: &[u8],
    ) -> Operation {
        let body = Body::new(payload);
        let mut header = Header {
            version: 1,
            verifying_key: signing_key.verifying_key(),
            signature: None,
            payload_size: body.size(),
            payload_hash: Some(body.hash()),
            timestamp: Timestamp::new(seq_num),
            seq_num,
            backlink,
            extensions: Extensions {
                log_id,
                prune_flag: PruneFlag::default(),
                groups_args: None,
                version: 1,
            },
        };
        header.sign(signing_key);
        Operation {
            hash: header.hash(),
            header,
            body: Some(body),
        }
    }

    async fn insert(store: &OpStore, op: &Operation, log_id: &LogId) {
        let permit = store.store.begin().await.unwrap();
        OperationStore::<Operation, Hash, LogId>::insert_operation(
            &store.store,
            &op.hash,
            op,
            log_id,
        )
        .await
        .unwrap();
        store.store.commit(permit).await.unwrap();
    }

    /// Mailbox sync must only see the contiguous prefix of a log whose
    /// operations have completed application-layer processing.
    #[tokio::test]
    async fn mailbox_get_log_truncates_at_first_unprocessed_op() {
        use mailbox_client::store::MailboxStore;

        let store = OpStore::temporary_sqlite().await.unwrap();
        let topic = TopicId::random();
        let log_id = LogId::from_topic(topic);
        let signing_key = p2panda::SigningKey::generate();
        let author = DeviceId::from(signing_key.verifying_key());

        let op0 = signed_op(&signing_key, log_id, 0, None, b"zero");
        let op1 = signed_op(&signing_key, log_id, 1, Some(op0.hash), b"one");
        insert(&store, &op0, &log_id).await;
        insert(&store, &op1, &log_id).await;

        let served = |store: &OpStore| {
            let store = store.clone();
            async move {
                MailboxStore::get_log(&store, &author, &topic, 0)
                    .await
                    .unwrap()
                    .unwrap()
                    .into_iter()
                    .map(|op| op.header.hash())
                    .collect::<Vec<_>>()
            }
        };

        assert_eq!(served(&store).await, vec![]);

        store.mark_op_processed_persistent(&op1.hash).await.unwrap();
        // op0 is still unprocessed, so nothing is served despite op1's marker.
        assert_eq!(served(&store).await, vec![]);

        store.mark_op_processed_persistent(&op0.hash).await.unwrap();
        assert_eq!(served(&store).await, vec![op0.hash, op1.hash]);
    }

    #[tokio::test]
    async fn delete_body_drops_payload_keeps_header() {
        let store = OpStore::temporary_sqlite().await.unwrap();
        let topic = TopicId::random();
        let log_id = LogId::from_topic(topic);

        let signing_key = p2panda::SigningKey::generate();
        let body = Body::new(b"payload");
        let mut header = Header {
            version: 1,
            verifying_key: signing_key.verifying_key(),
            signature: None,
            payload_size: body.size(),
            payload_hash: Some(body.hash()),
            timestamp: Timestamp::new(0),
            seq_num: 0,
            backlink: None,
            extensions: Extensions {
                log_id,
                prune_flag: PruneFlag::default(),
                groups_args: None,
                version: 1,
            },
        };
        header.sign(&signing_key);
        let hash = header.hash();
        let op = Operation {
            hash,
            header,
            body: Some(body),
        };

        let permit = store.store.begin().await.unwrap();
        OperationStore::<Operation, Hash, LogId>::insert_operation(
            &store.store,
            &hash,
            &op,
            &log_id,
        )
        .await
        .unwrap();
        store.store.commit(permit).await.unwrap();
        assert!(fetch(&store, &hash).await.body.is_some());

        store.delete_body(&hash).await.unwrap();

        let stored = fetch(&store, &hash).await;
        assert!(stored.body.is_none());
        // The header is retained so log sync stays consistent.
        assert_eq!(stored.header.seq_num, 0);
    }
}
