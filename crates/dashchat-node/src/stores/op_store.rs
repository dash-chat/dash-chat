pub mod queries;

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::{Arc, RwLock},
};

use p2panda_core::{Hash, SeqNum};
use p2panda_store::{SqliteStore, logs::LogStore};

use tokio::sync::Mutex;

use crate::{
    mailbox::MailboxOperation,
    payload::Extensions,
    topic::{Topic, TopicId, TopicKind},
    util::first,
    *,
};

#[derive(Clone, derive_more::Deref, derive_more::DerefMut)]
pub struct OpStore<S = SqliteStore> {
    #[deref]
    #[deref_mut]
    pub(crate) store: S,
    pub processed_ops: Arc<RwLock<HashMap<TopicId, HashSet<Hash>>>>,
    write_mutex: Arc<Mutex<()>>,
    /// Handle to the SQLite pool,
    /// so `close()` can release the database file handles on shutdown.
    sqlite_pool: sqlx::SqlitePool,
}

impl OpStore {
    pub async fn new_sqlite(
        database_file_path: impl AsRef<std::path::Path>,
    ) -> anyhow::Result<Self> {
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
        let store = SqliteStore::from_pool(pool.clone());
        Ok(Self {
            store,
            write_mutex: Arc::new(Mutex::new(())),
            processed_ops: Arc::new(RwLock::new(HashMap::new())),
            sqlite_pool: pool,
        })
    }

    /// Gracefully close the underlying SQLite pool (no-op for the in-memory variant).
    pub async fn close(&self) {
        self.sqlite_pool.close().await;
    }

    pub async fn get_log(
        &self,
        author: &DeviceId,
        topic: &TopicId,
        from: Option<u64>,
    ) -> anyhow::Result<Vec<Operation>> {
        let log = self
            .store
            .get_log_entries(author, topic, from, None)
            .await?
            .unwrap_or_else(|| {
                tracing::warn!("No log found for topic {topic:?} and author {author:?}");
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
        topic: &TopicId,
    ) -> Result<BTreeMap<DeviceId, SeqNum>, anyhow::Error> {
        queries::get_log_heights_by_author(&self.store, topic).await
    }

    pub async fn author_operation<K: TopicKind>(
        &self,
        private_key: &PrivateKey,
        topic: Topic<K>,
        payload: DashAction,
        alias: Option<&str>,
    ) -> Result<Operation, anyhow::Error> {
        let device_id = DeviceId::from(private_key.public_key());
        let topic = topic.clone();

        let body = payload.try_into_body()?;

        let lock = self.write_mutex.lock().await;
        let latest_operation: Option<Operation> =
            self.store.get_latest_entry(&device_id, &*topic).await?;

        let (seq_num, backlink) = match latest_operation {
            Some(op) => (op.header.seq_num + 1, Some(op.hash)),
            None => (0, None),
        };

        let extensions = Extensions {
            topic: topic.clone().into(),
            auth: payload.extract_auth_extension(),
        };

        let timestamp = Timestamp::now();

        let mut header = Header {
            version: 1,
            public_key: *device_id,
            signature: None,
            payload_size: body.as_ref().map_or(0, |body| body.size()),
            payload_hash: body.as_ref().map(|body| body.hash()),
            timestamp,
            seq_num,
            backlink,
            extensions,
        };

        header.sign(private_key);

        let topic = header.extensions.topic;
        let hash = header.hash();

        if let Some(alias) = alias {
            header.hash().with_name(alias);
        } else {
            header.hash().with_serial();
        }

        tracing::info!(
            topic = ?topic.renamed(),
            hash = ?hash.renamed(),
            seq_num = header.seq_num,
            "PUB: authoring operation"
        );

        let operation = Operation {
            hash,
            header: header.clone(),
            body,
        };

        let new = p2panda_stream::ingest::ingest_operation(
            &mut *self.clone(),
            &operation,
            &*topic,
            &topic,
            false,
        )
        .await?;

        if new {
            self.mark_op_processed(topic, &hash);
        }

        // Let the next op be authored as soon as this one's ingested
        drop(lock);

        Ok(operation)
    }

    pub fn mark_op_processed(&self, topic: TopicId, hash: &Hash) {
        self.processed_ops
            .write()
            .unwrap()
            .entry(topic)
            .or_default()
            .insert(hash.clone());
    }

    pub fn is_op_processed(&self, topic: &TopicId, hash: &Hash) -> bool {
        self.processed_ops
            .read()
            .unwrap()
            .get(topic)
            .map(|s| s.contains(hash))
            .unwrap_or(false)
    }
}

impl OpStore<SqliteStore> {
    pub fn report<'a>(&self, _topics: impl IntoIterator<Item = &'a TopicId>) -> String {
        tracing::warn!("report() not implemented for SqliteStore");
        format!("report() not implemented for SqliteStore")
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
        let from = if from == 0 { None } else { Some(from - 1) };
        let log = self
            .store
            .get_log_entries(author, topic, from, None)
            .await
            .map_err(|err| anyhow::anyhow!("failed to get log for {author:?}: {topic:?}: {err}"))?;

        Ok(log.map(|log| {
            log.into_iter()
                .map(|(op, _)| MailboxOperation {
                    header: op.header,
                    body: op.body,
                })
                .collect()
        }))
    }

    async fn get_log_heights(&self, topic: &TopicId) -> anyhow::Result<Vec<(DeviceId, u64)>> {
        Ok(OpStore::get_log_heights(self, topic)
            .await?
            .into_iter()
            .collect())
    }
}
