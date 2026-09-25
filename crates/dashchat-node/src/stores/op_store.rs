pub mod queries;

use std::collections::{BTreeMap, HashSet};
#[cfg(feature = "testing")]
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use dashchat_utils::SeqNum;
use futures::TryStreamExt;
use p2panda::Hash;
#[cfg(any(test, feature = "testing"))]
use p2panda::operation::Header;
use p2panda::operation::{LogId, Operation};
use p2panda_store::SqliteStore;
use p2panda_store::logs::LogStore;

use crate::{mailbox::MailboxOperation, topic::TopicId, *};

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
            #[cfg(feature = "testing")]
            processed_ops: Arc::new(RwLock::new(HashMap::new())),
        };
        Ok(store)
    }

    pub fn from_sqlite(store: SqliteStore) -> Self {
        Self {
            store,
            #[cfg(feature = "testing")]
            processed_ops: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    #[cfg(feature = "testing")]
    pub async fn temporary_sqlite() -> anyhow::Result<Self> {
        let store = SqliteStore::temporary().await;
        Ok(Self::from_sqlite(store))
    }

    /// Highest sequence number the node has acknowledged for `author`'s log in
    /// `topic`, or `None` if nothing has been acknowledged yet.
    ///
    /// Under the node's `Explicit` ack policy an operation is acknowledged only
    /// once application-layer processing has finished (see
    /// `Node::spawn_application_processor_task`), so p2panda's persisted ack
    /// cursor is exactly the "processed" watermark that gates mailbox
    /// transmission — an operation whose payload might still be tombstoned by
    /// pending processing sits above the watermark and is never sent onward.
    pub(crate) async fn acked_log_height(
        &self,
        topic: &TopicId,
        author: &DeviceId,
        log_id: &LogId,
    ) -> anyhow::Result<Option<SeqNum>> {
        use p2panda_store::cursors::CursorStore;
        // The ack cursor is persisted by p2panda under the topic's string
        // representation (see `StreamSubscription`'s internal `Acked`).
        let cursor =
            CursorStore::<p2panda::VerifyingKey, LogId>::get_cursor(&self.store, topic.to_string())
                .await?;
        Ok(cursor.and_then(|c| c.log_height(author, log_id).copied()))
    }

    /// Every sequence number present for `author`'s log, ascending. The
    /// router's held-range input (see `lan_router.rs`).
    #[cfg(any(test, feature = "lan-router"))]
    pub(crate) async fn get_log_seqs(
        &self,
        author: &DeviceId,
        log_id: &LogId,
    ) -> anyhow::Result<Vec<SeqNum>> {
        queries::get_log_seqs(&self.store, author, log_id).await
    }

    /// Gracefully close the underlying SQLite pool (no-op for the in-memory variant).
    pub async fn close(&self) {
        self.store.pool().close().await;
    }

    pub async fn get_log(
        &self,
        author: &DeviceId,
        log_id: &LogId,
        from: Option<SeqNum>,
    ) -> anyhow::Result<Vec<Operation>> {
        let log = self.log_operations(author, log_id, from).await?;
        if log.is_empty() && !self.log_exists(author, log_id).await? {
            tracing::warn!(
                "No log found for log_id {} and author {}",
                Hash::from_bytes(*log_id.as_bytes()),
                author
            );
        }
        Ok(log)
    }

    /// Whether we hold any entry of `author`'s log, telling an absent log apart
    /// from one with nothing past a cursor.
    async fn log_exists(&self, author: &DeviceId, log_id: &LogId) -> anyhow::Result<bool> {
        let heights = self
            .store
            .get_log_heights(author, std::slice::from_ref(log_id))
            .await?;
        Ok(heights.is_some())
    }

    /// Collect a log's entries, decoding each stored operation into our extension type.
    async fn log_operations(
        &self,
        author: &DeviceId,
        log_id: &LogId,
        from: Option<SeqNum>,
    ) -> anyhow::Result<Vec<Operation>> {
        self.store
            .log_entries(author, log_id, from, None)?
            .map_err(anyhow::Error::from)
            .and_then(|entry| async move { Ok(Operation::try_from(entry.entry)?) })
            .try_collect()
            .await
    }

    pub async fn get_operation(&self, hash: &Hash) -> anyhow::Result<Option<Operation>> {
        use p2panda_store::operations::OperationStore;
        OperationStore::<Operation, Hash>::get_operation(&self.store, hash)
            .await
            .map_err(|err| anyhow::anyhow!("failed to get operation for {hash:?}: {err}"))
    }

    /// Whether an operation is stored, with or without its body.
    #[cfg(feature = "lan-router")]
    pub(crate) async fn has_operation(&self, hash: &Hash) -> anyhow::Result<bool> {
        use p2panda_store::operations::OperationStore;
        OperationStore::<Operation, Hash>::has_operation(&self.store, hash)
            .await
            .map_err(|err| anyhow::anyhow!("failed to look up operation {hash:?}: {err}"))
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
        logs.sort_by_key(|(h, _)| h.extensions.timestamp());
        Ok(logs)
    }

    /// Drop the stored payload (body) of an operation, leaving its header
    /// intact so log sync stays consistent. Used to enforce tombstones.
    pub async fn delete_body(&self, hash: &Hash) -> anyhow::Result<()> {
        use p2panda_store::operations::OperationStore;
        OperationStore::<Operation, Hash>::delete_operation_payload(&self.store, hash).await?;
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
}

#[async_trait::async_trait]
impl mailbox_client::store::MailboxStore<MailboxOperation> for OpStore {
    async fn get_log(
        &self,
        author: &DeviceId,
        topic: &TopicId,
        from: SeqNum,
    ) -> Result<Option<Vec<MailboxOperation>>, anyhow::Error> {
        let log_id = LogId::from_topic(*topic);
        let from = from.checked_sub(1);
        let log = self.log_operations(author, &log_id, from).await?;
        if log.is_empty() && !self.log_exists(author, &log_id).await? {
            return Ok(None);
        }

        // Only transmit the contiguous prefix of fully-processed operations.
        // An operation that hasn't completed application-layer processing may
        // still have its payload dropped by a tombstone it is about to
        // enforce, so it must not be sent onward yet. Truncating (rather than
        // filtering) keeps the returned log dense from `from`, which callers
        // index by sequence number. Body-less operations are always safe to
        // transmit — there is no payload to leak — and are acknowledged by
        // p2panda before ever reaching the application layer.
        let acked_height = self.acked_log_height(topic, author, &log_id).await?;
        let mut ops = Vec::with_capacity(log.len());
        for op in log {
            if op.body.is_some() && acked_height.is_none_or(|h| op.header.seq_num > h) {
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

    async fn get_log_heights(&self, topic: &TopicId) -> anyhow::Result<Vec<(DeviceId, SeqNum)>> {
        Ok(OpStore::get_log_heights(self, &LogId::from_topic(*topic))
            .await?
            .into_iter()
            .collect())
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use p2panda::operation::{Extensions, Header};
    use p2panda_core::{Body, Timestamp};
    use p2panda_store::Transaction;
    use p2panda_store::operations::OperationStore;

    use super::*;

    pub(crate) fn signed_op(
        signing_key: &p2panda::SigningKey,
        log_id: LogId,
        seq_num: SeqNum,
        backlink: Option<Hash>,
        payload: &[u8],
    ) -> Operation {
        let body = Body::from_bytes(payload);
        // Timestamps track seq_num so the tests' expected ordering is deterministic.
        let extensions = Extensions::builder(log_id)
            .timestamp(Timestamp::new(seq_num.into()))
            .build();
        let header = Header::builder()
            .body(payload)
            .seq_num(seq_num)
            .backlink(backlink)
            .build(signing_key, extensions);
        Operation {
            hash: header.hash(),
            header,
            body: Some(body),
        }
    }

    pub(crate) async fn insert(store: &OpStore, op: &Operation, log_id: &LogId) {
        let permit = store.store.begin().await.unwrap();
        OperationStore::<Operation, Hash>::insert_operation(&store.store, &op.hash, op, log_id)
            .await
            .unwrap();
        store.store.commit(permit).await.unwrap();
    }

    /// Advance p2panda's ack cursor for `author`'s log to `seq`, mimicking what
    /// `ProcessedOperation::ack` persists once application-layer processing has
    /// finished (see `OpStore::acked_log_height`).
    pub(crate) async fn ack_up_to(
        store: &OpStore,
        topic: &TopicId,
        author: &DeviceId,
        log_id: LogId,
        seq: SeqNum,
    ) {
        use p2panda_core::Cursor;
        use p2panda_core::logs::LogHeights;
        use p2panda_store::cursors::CursorStore;

        let mut cursor = CursorStore::<p2panda::VerifyingKey, LogId>::get_cursor(
            &store.store,
            topic.to_string(),
        )
        .await
        .unwrap()
        .unwrap_or_else(|| Cursor::new(topic.to_string(), LogHeights::default()));
        cursor.advance(**author, log_id, seq);
        let permit = store.store.begin().await.unwrap();
        CursorStore::set_cursor(&store.store, &cursor)
            .await
            .unwrap();
        store.store.commit(permit).await.unwrap();
    }
}

#[cfg(test)]
mod tests {
    use p2panda::operation::{Extensions, Header};
    use p2panda_core::Body;
    use p2panda_store::Transaction;
    use p2panda_store::operations::OperationStore;

    use super::test_support::*;
    use super::*;

    async fn fetch(store: &OpStore, hash: &Hash) -> Operation {
        OperationStore::<Operation, Hash>::get_operation(&store.store, hash)
            .await
            .unwrap()
            .unwrap()
    }

    #[tokio::test]
    async fn mailbox_get_log_distinguishes_absent_from_up_to_date() {
        use mailbox_client::store::MailboxStore;

        let store = OpStore::temporary_sqlite().await.unwrap();
        let topic = TopicId::random();
        let log_id = LogId::from_topic(topic);
        let signing_key = p2panda::SigningKey::generate();
        let author = DeviceId::from(signing_key.verifying_key());

        for from in [0, 1] {
            let log = MailboxStore::get_log(&store, &author, &topic, from)
                .await
                .unwrap();
            assert!(log.is_none(), "absent log from {from}: {log:?}");
        }

        insert(
            &store,
            &signed_op(&signing_key, log_id, 0, None, b"zero"),
            &log_id,
        )
        .await;
        let log = MailboxStore::get_log(&store, &author, &topic, 1)
            .await
            .unwrap();
        assert_eq!(log.map(|ops| ops.len()), Some(0));
    }

    /// Mailbox sync must only see the contiguous prefix of a log whose
    /// operations have completed application-layer processing, as recorded by
    /// p2panda's ack cursor.
    #[tokio::test]
    async fn mailbox_get_log_truncates_at_first_unacked_op() {
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

        // Nothing acked yet: no body-carrying op may be transmitted.
        assert_eq!(served(&store).await, vec![]);

        ack_up_to(&store, &topic, &author, log_id, 0).await;
        // Only op0 is acked; op1 sits above the watermark and truncates the log.
        assert_eq!(served(&store).await, vec![op0.hash]);

        ack_up_to(&store, &topic, &author, log_id, 1).await;
        assert_eq!(served(&store).await, vec![op0.hash, op1.hash]);

        // A body-less operation (tombstoned payload) carries no payload and is
        // always safe to transmit, even though it sits above the ack watermark.
        let mut op2 = signed_op(&signing_key, log_id, 2, Some(op1.hash), b"two");
        op2.body = None;
        insert(&store, &op2, &log_id).await;
        assert_eq!(served(&store).await, vec![op0.hash, op1.hash, op2.hash]);
    }

    #[tokio::test]
    async fn delete_body_drops_payload_keeps_header() {
        let store = OpStore::temporary_sqlite().await.unwrap();
        let topic = TopicId::random();
        let log_id = LogId::from_topic(topic);

        let signing_key = p2panda::SigningKey::generate();
        let body = Body::from_bytes(b"payload");
        let header = Header::builder()
            .body(b"payload")
            .build(&signing_key, Extensions::builder(log_id).build());
        let hash = header.hash();
        let op = Operation {
            hash,
            header,
            body: Some(body),
        };

        let permit = store.store.begin().await.unwrap();
        OperationStore::<Operation, Hash>::insert_operation(&store.store, &hash, &op, &log_id)
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

    #[tokio::test]
    async fn get_log_seqs_lists_present_seqs_ascending() {
        let store = OpStore::temporary_sqlite().await.unwrap();
        let key = p2panda::SigningKey::generate();
        let author = DeviceId::from(key.verifying_key());
        let log_id = LogId::from_topic(TopicId::random());
        let op0 = signed_op(&key, log_id, 0, None, b"a");
        let op1 = signed_op(&key, log_id, 1, Some(op0.hash), b"b");
        let op2 = signed_op(&key, log_id, 2, Some(op1.hash), b"c");
        insert(&store, &op2, &log_id).await;
        insert(&store, &op0, &log_id).await;
        insert(&store, &op1, &log_id).await;
        assert_eq!(
            store.get_log_seqs(&author, &log_id).await.unwrap(),
            vec![0, 1, 2]
        );
        let other = DeviceId::from(p2panda::SigningKey::generate().verifying_key());
        assert_eq!(
            store.get_log_seqs(&other, &log_id).await.unwrap(),
            Vec::<SeqNum>::new()
        );
    }
}
