use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, RwLock},
};

use p2panda_core::{Body, Hash};
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
    // pub orderer: Arc<tokio::sync::RwLock<Orderer<S>>>,
    pub processed_ops: Arc<RwLock<HashMap<TopicId, HashSet<Hash>>>>,
    write_mutex: Arc<Mutex<()>>,
}

impl OpStore {
    pub fn new(store: SqliteStore) -> Self {
        // let orderer = Arc::new(tokio::sync::RwLock::new(Orderer::new(
        //     store.clone(),
        //     Default::default(),
        // )));

        Self {
            store,
            // orderer,
            write_mutex: Arc::new(Mutex::new(())),
            processed_ops: Arc::new(RwLock::new(HashMap::new())),
        }
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
    ) -> Result<Vec<(DeviceId, u64)>, anyhow::Error> {
        todo!("need new log heights query to match the old one")
        // Ok(self
        //     .store
        //     .get_log_heights(&topic)
        //     .await
        //     .map_err(|err| anyhow::anyhow!("failed to get log heights for {topic:?}: {err}"))?
        //     .into_iter()
        //     .map(|(pk, height)| (DeviceId::from(pk), height))
        //     .collect::<Vec<_>>())
    }

    // async fn get_log_heights(
    //     &self,
    //     author: &PublicKey,
    //     logs: &[L],
    // ) -> Result<Option<BTreeMap<L, SeqNum>>, Self::Error> {
    //     let mut encoded_log_ids = Vec::new();
    //     for log in logs {
    //         let encoded_log_id =
    //             encode_cbor(&log).map_err(|err| SqliteError::Encode("log id".to_string(), err))?;
    //         encoded_log_ids.push(encoded_log_id);
    //     }

    //     // This query formation approach is required since there is currently no
    //     // way to directly bind arrays as comma-separated lists in sqlx.
    //     let params = format!("?{}", ", ?".repeat(encoded_log_ids.len() - 1));
    //     let query_str = format!(
    //         "
    //         SELECT
    //             log_id,
    //             CAST(MAX(CAST(seq_num AS NUMERIC)) AS TEXT) as seq_num
    //         FROM
    //             operations_v1
    //         WHERE
    //             public_key = ?
    //             AND log_id IN ( {} )
    //         GROUP BY
    //             log_id
    //         ",
    //         params
    //     );

    //     let mut query = query_as::<_, LogHeightRow>(&query_str).bind(author.to_string());

    //     for log_id in encoded_log_ids {
    //         query = query.bind(log_id)
    //     }

    //     let log_heights_query = query.fetch_all(&self.pool).await?;

    //     let log_heights = if log_heights_query.is_empty() {
    //         None
    //     } else {
    //         let mut log_heights = BTreeMap::new();

    //         for row in log_heights_query {
    //             let (log_id, seq_num) = row.try_into()?;
    //             log_heights.insert(log_id, seq_num);
    //         }

    //         Some(log_heights)
    //     };

    //     Ok(log_heights)
    // }

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

        // TODO: is this the place to integrate group auth processing?

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
            // self.process_ordering(op.clone()).await?;
            self.mark_op_processed(topic, &hash);
        }

        // Let the next op be authored as soon as this one's ingested
        drop(lock);

        Ok(operation)
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

impl OpStore<SqliteStore> {
    pub fn report<'a>(&self, _topics: impl IntoIterator<Item = &'a TopicId>) -> String {
        tracing::warn!("report() not implemented for SqliteStore");
        format!("report() not implemented for SqliteStore")
    }
}

// impl OpStore<MemoryStore<TopicId, Extensions>> {
//     pub fn report<'a>(&self, topics: impl IntoIterator<Item = &'a TopicId>) -> String {
//         let topics = topics.into_iter().collect::<Vec<_>>();
//         let s = self.store.read_store();
//         let mut ops = s
//             .operations
//             .iter()
//             .filter(|(_, (l, _, _, _))| {
//                 topics.is_empty() || topics.iter().find(|topic| **topic == l).is_some()
//             })
//             .collect::<Vec<_>>();
//         ops.sort_by_key(|(_, (t, header, _, _))| (t, header.public_key.renamed(), header.seq_num));
//         ops.into_iter()
//             .map(|(h, (t, header, body, _))| {
//                 let desc = match body
//                     .clone()
//                     .map(|body| Payload::try_from_body(&body).unwrap())
//                 {
//                     // Some(Payload::Space(args)) => {
//                     //     let space_op = GroupOp::new(header.clone(), args);
//                     //     format!("{:?}", space_op.arg_type())
//                     // }
//                     Some(p) => format!("{p:?}"),
//                     None => "_".to_string(),
//                 };
//                 if topics.len() == 1 {
//                     format!(
//                         "• {} {:2} {} : {}",
//                         header.public_key.renamed(),
//                         header.seq_num,
//                         h.renamed(),
//                         desc
//                     )
//                 } else {
//                     let t = format!("{t:?}");
//                     format!(
//                         "• {:>24} {} {:2} {} : {}",
//                         t,
//                         header.public_key.renamed(),
//                         header.seq_num,
//                         h.renamed(),
//                         desc
//                     )
//                 }
//             })
//             .collect::<Vec<_>>()
//             .join("\n")
//     }
// }

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
                .map(|(op, _)| MailboxOperation {
                    header: op.header,
                    body: op.body,
                })
                .collect()
        }))
    }

    async fn get_log_heights(&self, topic: &TopicId) -> anyhow::Result<Vec<(DeviceId, u64)>> {
        OpStore::get_log_heights(self, topic).await
    }
}
