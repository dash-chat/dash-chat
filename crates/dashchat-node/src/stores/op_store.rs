use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{Arc, RwLock},
};

use p2panda_core::{Hash, Operation};
use p2panda_store::{SqliteStore, logs::LogStore};
use tokio::sync::Mutex;

use crate::{
    mailbox::MailboxOperation,
    payload::{Extensions, Payload},
    topic::{Topic, TopicId, TopicKind},
    util::first,
    *,
};

#[derive(Clone, derive_more::Deref, derive_more::DerefMut)]
pub struct OpStore {
    #[deref]
    #[deref_mut]
    pub(crate) store: SqliteStore,
    // pub orderer: Arc<tokio::sync::RwLock<Orderer<S>>>,
    pub processed_ops: Arc<RwLock<HashMap<TopicId, HashSet<Hash>>>>,
    write_mutex: Arc<Mutex<()>>,
}

impl OpStore {
    pub async fn create(database_file_path: PathBuf) -> anyhow::Result<Self> {
        let url = format!("sqlite://{}", database_file_path.to_string_lossy());
        p2panda_store::sqlite::create_database(&url).await?;

        let pool = sqlx::SqlitePool::connect(&url).await.map_err(|e| {
            anyhow::anyhow!("failed to connect to sqlite at '{database_file_path:?}': {e}")
        })?;

        if p2panda_store::sqlite::run_pending_migrations(&pool)
            .await
            .is_err()
        {
            pool.close().await;
            panic!("Database migration failed");
        }
        let store = SqliteStore::from_pool(pool);

        Ok(Self::new(store))
    }

    pub fn new(store: SqliteStore) -> Self {
        // let orderer = Arc::new(tokio::sync::RwLock::new(Orderer::new(
        //     store.clone(),
        //     Default::default(),
        // )));

        Self {
            store,
            write_mutex: Arc::new(Mutex::new(())),
            processed_ops: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get the "height" of each log, which is actually the highest sequence number of the log.
    pub async fn get_log_heights(
        &self,
        topic: &TopicId,
    ) -> Result<Vec<(DeviceId, u64)>, anyhow::Error> {
        todo!("need log heights for all authors")
        // Ok(self
        //     .store
        //     .get_log_heights(&topic)
        //     .await
        //     .map_err(|err| anyhow::anyhow!("failed to get log heights for {topic:?}: {err}"))?
        //     .into_iter()
        //     .map(|(pk, height)| (DeviceId::from(pk), height))
        //     .collect::<Vec<_>>())
    }

    pub async fn author_operation<K: TopicKind>(
        &self,
        private_key: &PrivateKey,
        topic: Topic<K>,
        payload: DashAction,
        alias: Option<&str>,
    ) -> Result<Operation<Extensions>, anyhow::Error> {
        let device_id = DeviceId::from(private_key.public_key());
        let topic = topic.clone();

        let body = payload.try_into_body()?;

        let lock = self.write_mutex.lock().await;
        let latest_operation: Option<Operation<Extensions>> =
            self.get_latest_entry(&device_id, &topic).await?;

        let (seq_num, backlink, last_time) = match latest_operation {
            Some(op) => (op.header.seq_num + 1, Some(op.hash), op.header.timestamp),
            None => (0, None, Timestamp::from(0)),
        };

        // TODO: is this the place to integrate group auth processing?

        let extensions = Extensions {
            topic: topic.clone().into(),
            hacky_group: payload.extract_hacky_group_extension(),
        };

        let timestamp = Timestamp::now();

        #[cfg(feature = "testing")]
        let timestamp = if timestamp <= last_time {
            tracing::warn!("timestamp is less than last operation timestamp, incrementing by 1");
            Timestamp::from(last_time.micros() + 1)
        } else {
            timestamp
        };

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

        tracing::debug!(
            topic = ?topic.renamed(),
            hash = ?hash.renamed(),
            seq_num = header.seq_num,
            "PUB: authoring operation"
        );

        let operation = Operation { hash, header, body };

        let inserted = p2panda_stream::ingest::ingest_operation(
            &mut *self.clone(),
            &operation,
            &topic.into(),
            &topic.into(),
            false,
        )
        .await?;

        if !inserted {
            tracing::warn!("operation already exists: {}", hash.renamed());
        }

        // Let the next op be authored as soon as this one's ingested
        drop(lock);

        Ok(operation)
    }

    pub async fn get_log_entries(
        &self,
        author: &DeviceId,
        topic: &TopicId,
        from: Option<u64>,
        until: Option<u64>,
    ) -> anyhow::Result<Option<Vec<Operation<Extensions>>>> {
        Ok(self
            .store
            .get_log_entries(author, topic, from, until)
            .await?
            .map(|ops| ops.into_iter().map(first).collect()))
    }

    // // SAM: could be generic https://github.com/p2panda/p2panda/blob/65727c7fff64376f9d2367686c2ed5132ff7c4e0/p2panda-stream/src/ordering/partial/mod.rs#L83
    // pub async fn process_ordering(&self, operation: Operation<Extensions>) -> anyhow::Result<()> {
    //     self.orderer.write().await.process(operation).await?;
    //     Ok(())
    // }

    // pub async fn next_ordering(&self) -> anyhow::Result<Vec<Operation<Extensions>>> {
    //     let mut ordering = self.orderer.write().await;
    //     let mut next = vec![];
    //     while let Some(op) = ordering.next().await? {
    //         next.push(op);
    //     }
    //     Ok(next)
    // }

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

impl OpStore {
    pub fn report<'a>(&self, _topics: impl IntoIterator<Item = &'a TopicId>) -> String {
        format!("( report() is only implemented for MemoryStore )")
    }
}

impl OpStore<MemoryStore<TopicId, Extensions>> {
    pub fn report<'a>(&self, topics: impl IntoIterator<Item = &'a TopicId>) -> String {
        let topics = topics.into_iter().collect::<Vec<_>>();
        let s = self.store.read_store();
        let mut ops = s
            .operations
            .iter()
            .filter(|(_, (l, _, _, _))| {
                topics.is_empty() || topics.iter().find(|topic| **topic == l).is_some()
            })
            .collect::<Vec<_>>();

        ops.sort_by_key(|(_, (t, header, _, _))| {
            (t, header.public_key.renamed().to_string(), header.seq_num)
        });

        ops.into_iter()
            .map(|(h, (t, header, body, _))| {
                let desc = match body
                    .clone()
                    .map(|body| Payload::try_from_body(&body).unwrap())
                {
                    // Some(Payload::Space(args)) => {
                    //     let space_op = GroupOp::new(header.clone(), args);
                    //     format!("{:?}", space_op.arg_type())
                    // }
                    Some(p) => format!("{p:?}"),
                    None => "_".to_string(),
                };
                let width = crate::util::max_width(&topics);
                format!(
                    "• {:>width$} {} {:2} {} : {}",
                    t.renamed(),
                    header.public_key.renamed(),
                    header.seq_num,
                    h.renamed(),
                    desc
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
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
        let log = self
            .store
            .get_log_entries(author, topic, Some(from), None)
            .await
            .map_err(|err| anyhow::anyhow!("failed to get log for {author:?}: {topic:?}: {err}"))?;
        Ok(log.map(|log| {
            log.into_iter()
                .map(|(header, body)| MailboxOperation { header, body })
                .collect()
        }))
    }

    async fn get_log_heights(&self, topic: &TopicId) -> anyhow::Result<Vec<(DeviceId, u64)>> {
        OpStore::get_log_heights(self, topic).await
    }
}
