use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{Arc, RwLock},
};

use p2panda_core::{Body, Hash, Operation, PublicKey, RawOperation};
use p2panda_store::{LogStore, MemoryStore, OperationStore, SqliteStore};
use p2panda_stream::operation::IngestResult;
use tokio::sync::Mutex;

use crate::{
    mailbox::MailboxOperation,
    node::Orderer,
    payload::{Extensions, Payload},
    topic::{Topic, TopicId, TopicKind},
    *,
};

#[derive(Clone, derive_more::Deref, derive_more::DerefMut)]
pub struct OpStore<S>
where
    S: OperationStore<TopicId, Extensions> + LogStore<TopicId, Extensions>,
    S: Send + Sync,
{
    #[deref]
    #[deref_mut]
    pub(crate) store: S,
    pub orderer: Arc<tokio::sync::RwLock<Orderer<S>>>,
    pub processed_ops: Arc<RwLock<HashMap<TopicId, HashSet<Hash>>>>,
    write_mutex: Arc<Mutex<()>>,
}

impl OpStore<MemoryStore<TopicId, Extensions>> {
    pub fn new_memory() -> Self {
        let store = MemoryStore::new();
        Self::new(store)
    }
}

impl OpStore<SqliteStore<TopicId, Extensions>> {
    pub async fn new_sqlite(database_file_path: PathBuf) -> anyhow::Result<Self> {
        let url = format!("sqlite://{}", database_file_path.to_string_lossy());
        p2panda_store::sqlite::store::create_database(&url).await?;

        let pool = sqlx::SqlitePool::connect(&url).await.map_err(|e| {
            anyhow::anyhow!("failed to connect to sqlite at '{database_file_path:?}': {e}")
        })?;

        if p2panda_store::sqlite::store::run_pending_migrations(&pool)
            .await
            .is_err()
        {
            pool.close().await;
            panic!("Database migration failed");
        }
        let store = SqliteStore::new(pool);

        Ok(Self::new(store))
    }
}

impl<S> OpStore<S>
where
    S: OperationStore<TopicId, Extensions> + LogStore<TopicId, Extensions>,
    S: Send + Sync,
{
    pub fn new(store: S) -> Self {
        let orderer = Arc::new(tokio::sync::RwLock::new(Orderer::new(
            store.clone(),
            Default::default(),
        )));

        Self {
            store,
            orderer,
            write_mutex: Arc::new(Mutex::new(())),
            processed_ops: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn get_log_heights(
        &self,
        topic: &TopicId,
    ) -> Result<Vec<(DeviceId, u64)>, anyhow::Error> {
        Ok(self
            .store
            .get_log_heights(&topic)
            .await
            .map_err(|err| anyhow::anyhow!("failed to get log heights for {topic:?}: {err}"))?
            .into_iter()
            .map(|(pk, height)| (DeviceId::from(pk), height))
            .collect::<Vec<_>>())
    }

    pub async fn author_operation<K: TopicKind>(
        &self,
        private_key: &PrivateKey,
        topic: Topic<K>,
        payload: Payload,
        deps: Vec<p2panda_core::Hash>,
        alias: Option<&str>,
    ) -> Result<(Header, Option<Body>), anyhow::Error> {
        let device_id = DeviceId::from(private_key.public_key());
        let topic = topic.clone();

        let body = Some(payload.try_into_body()?);

        let extensions = Extensions {
            topic: topic.clone().into(),
        };

        let lock = self.write_mutex.lock().await;
        let latest_operation = self
            .latest_operation(&device_id, &topic.into())
            .await
            .unwrap();

        let (seq_num, backlink) = match latest_operation {
            Some((header, _)) => (header.seq_num + 1, Some(header.hash())),
            None => (0, None),
        };

        let timestamp = timestamp_now();

        let mut header = Header {
            version: 1,
            public_key: *device_id,
            signature: None,
            payload_size: body.as_ref().map_or(0, |body| body.size()),
            payload_hash: body.as_ref().map(|body| body.hash()),
            timestamp,
            seq_num,
            backlink,
            previous: deps,
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

        let result = p2panda_stream::operation::ingest_operation(
            &mut *self.clone(),
            header.clone(),
            body.clone(),
            header.to_bytes(),
            &topic.into(),
            false,
        )
        .await?;

        match result {
            IngestResult::Complete(op @ Operation { hash: hash2, .. }) => {
                assert_eq!(hash, hash2);

                // NOTE: if we fail to process here, incoming operations will be stuck as pending!
                self.process_ordering(op.clone()).await?;
            }

            IngestResult::Retry(h, _, _, missing) => {
                let backlink = h.backlink.as_ref().map(|h| h.renamed());
                tracing::error!(
                    ?topic,
                    hash = ?hash.renamed(),
                    ?backlink,
                    ?missing,
                    "operation could not be ingested"
                );
                panic!("operation could not be ingested, check your sequence numbers!");
            }

            IngestResult::Outdated(op) => {
                tracing::error!(?op, "operation is outdated");
                panic!("operation is outdated");
            }
        }

        // Let the next op be authored as soon as this one's ingested
        drop(lock);

        Ok((header, body))
    }

    // SAM: could be generic https://github.com/p2panda/p2panda/blob/65727c7fff64376f9d2367686c2ed5132ff7c4e0/p2panda-stream/src/ordering/partial/mod.rs#L83
    pub async fn process_ordering(&self, operation: Operation<Extensions>) -> anyhow::Result<()> {
        self.orderer.write().await.process(operation).await?;
        Ok(())
    }

    pub async fn next_ordering(&self) -> anyhow::Result<Vec<Operation<Extensions>>> {
        let mut ordering = self.orderer.write().await;
        let mut next = vec![];
        while let Some(op) = ordering.next().await? {
            next.push(op);
        }
        Ok(next)
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

impl OpStore<SqliteStore<TopicId, Extensions>> {
    pub fn report<'a>(&self, _topics: impl IntoIterator<Item = &'a TopicId>) -> String {
        tracing::warn!("report() not implemented for SqliteStore");
        format!("report() not implemented for SqliteStore")
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
        ops.sort_by_key(|(_, (t, header, _, _))| (t, header.public_key.renamed(), header.seq_num));
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
                if topics.len() == 1 {
                    format!(
                        "• {} {:2} {} : {}",
                        header.public_key.renamed(),
                        header.seq_num,
                        h.renamed(),
                        desc
                    )
                } else {
                    let t = format!("{t:?}");
                    format!(
                        "• {:>24} {} {:2} {} : {}",
                        t,
                        header.public_key.renamed(),
                        header.seq_num,
                        h.renamed(),
                        desc
                    )
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

impl<S> OperationStore<TopicId, Extensions> for OpStore<S>
where
    S: OperationStore<TopicId, Extensions> + LogStore<TopicId, Extensions> + Send + Sync,
    <S as OperationStore<TopicId, Extensions>>::Error: std::error::Error + Send + Sync,
{
    type Error = <S as OperationStore<TopicId, Extensions>>::Error;

    async fn insert_operation(
        &mut self,
        hash: Hash,
        header: &Header,
        body: Option<&Body>,
        header_bytes: &[u8],
        topic: &TopicId,
    ) -> Result<bool, Self::Error> {
        self.store
            .insert_operation(hash, header, body, header_bytes, topic)
            .await
    }

    async fn get_operation(
        &self,
        hash: Hash,
    ) -> Result<Option<(Header, Option<Body>)>, Self::Error> {
        self.store.get_operation(hash).await
    }

    async fn get_raw_operation(&self, hash: Hash) -> Result<Option<RawOperation>, Self::Error> {
        self.store.get_raw_operation(hash).await
    }

    async fn has_operation(&self, hash: Hash) -> Result<bool, Self::Error> {
        self.store.has_operation(hash).await
    }

    async fn delete_operation(&mut self, hash: Hash) -> Result<bool, Self::Error> {
        self.store.delete_operation(hash).await
    }

    async fn delete_payload(&mut self, hash: Hash) -> Result<bool, Self::Error> {
        self.store.delete_payload(hash).await
    }
}

impl<S> LogStore<TopicId, Extensions> for OpStore<S>
where
    S: LogStore<TopicId, Extensions>,
    S: OperationStore<TopicId, Extensions>,
    S: Send + Sync,
    <S as LogStore<TopicId, Extensions>>::Error: std::error::Error + Send + Sync,
{
    type Error = <S as LogStore<TopicId, Extensions>>::Error;

    async fn get_log(
        &self,
        public_key: &PublicKey,
        topic: &TopicId,
        from: Option<u64>,
    ) -> Result<Option<Vec<(Header, Option<Body>)>>, Self::Error> {
        self.store.get_log(public_key, topic, from).await
    }

    async fn get_raw_log(
        &self,
        public_key: &PublicKey,
        topic: &TopicId,
        from: Option<u64>,
    ) -> Result<Option<Vec<RawOperation>>, Self::Error> {
        self.store.get_raw_log(public_key, topic, from).await
    }

    async fn latest_operation(
        &self,
        public_key: &PublicKey,
        topic: &TopicId,
    ) -> Result<Option<(Header, Option<Body>)>, Self::Error> {
        self.store.latest_operation(public_key, topic).await
    }

    async fn get_log_heights(&self, topic: &TopicId) -> Result<Vec<(PublicKey, u64)>, Self::Error> {
        self.store.get_log_heights(topic).await
    }

    async fn delete_operations(
        &mut self,
        public_key: &PublicKey,
        topic: &TopicId,
        before: u64,
    ) -> Result<bool, Self::Error> {
        self.store
            .delete_operations(public_key, topic, before)
            .await
    }

    async fn delete_payloads(
        &mut self,
        public_key: &PublicKey,
        topic: &TopicId,
        from: u64,
        to: u64,
    ) -> Result<bool, Self::Error> {
        self.store
            .delete_payloads(public_key, topic, from, to)
            .await
    }

    async fn get_log_size(
        &self,
        public_key: &PublicKey,
        topic: &TopicId,
        from: Option<u64>,
    ) -> Result<Option<u64>, Self::Error> {
        self.store.get_log_size(public_key, topic, from).await
    }

    async fn get_log_hashes(
        &self,
        public_key: &PublicKey,
        topic: &TopicId,
        from: Option<u64>,
    ) -> Result<Option<Vec<Hash>>, Self::Error> {
        self.store.get_log_hashes(public_key, topic, from).await
    }
}

#[async_trait::async_trait]
impl<S> mailbox_client::store::MailboxStore<MailboxOperation> for OpStore<S>
where
    S: OperationStore<TopicId, Extensions> + LogStore<TopicId, Extensions>,
    S: Send + Sync + 'static,
{
    async fn get_log(
        &self,
        author: &DeviceId,
        topic: &TopicId,
        from: u64,
    ) -> Result<Option<Vec<MailboxOperation>>, anyhow::Error> {
        let log = self
            .store
            .get_log(author, topic, Some(from))
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
