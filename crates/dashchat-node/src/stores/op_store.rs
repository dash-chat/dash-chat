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
        Ok(Self {
            store,
            processed_ops: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    pub fn from_sqlite(store: SqliteStore) -> Self {
        Self {
            store,
            processed_ops: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn temporary_sqlite() -> anyhow::Result<Self> {
        let store = SqliteStore::temporary().await;
        Ok(Self::from_sqlite(store))
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

        Ok(log.map(|log| {
            log.into_iter()
                .map(|(op, _)| MailboxOperation {
                    topic: *topic,
                    header: op.header,
                    body: op.body,
                })
                .collect()
        }))
    }

    async fn get_log_heights(&self, topic: &TopicId) -> anyhow::Result<Vec<(DeviceId, u64)>> {
        Ok(OpStore::get_log_heights(self, &LogId::from_topic(*topic))
            .await?
            .into_iter()
            .collect())
    }
}
