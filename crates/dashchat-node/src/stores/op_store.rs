pub mod queries;

use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;
#[cfg(feature = "testing")]
use std::{collections::HashMap, sync::RwLock};

use dashchat_utils::SeqNum;
use futures::TryStreamExt;
use p2panda::Hash;
#[cfg(any(test, feature = "testing"))]
use p2panda::operation::Header;
use p2panda::operation::{ExtensionsVariantV1, LogId, Operation};
use p2panda_spaces::SpacesArgs;
use p2panda_store::SqliteStore;
use p2panda_store::logs::LogStore;
use p2panda_store::topics::TopicStore;

pub use crate::node::streams::Unreadable;
use crate::node::streams::{EncryptedMessage, TopicStreams};
use crate::{mailbox::MailboxOperation, topic::TopicId, *};

/// An operation as the app reads it, see [`OpStore::get_log`]. Never send one onward: mailbox
/// sync reads the stored operations instead.
#[derive(Clone, Debug)]
pub struct ReadOperation {
    pub hash: Hash,
    pub header: p2panda::operation::Header,
    /// The payload, decrypted if the operation was published into a space. `Ok(None)` is an
    /// operation that carries none: a control message, or one whose payload was deleted.
    pub body: Result<Option<p2panda_core::Body>, Unreadable>,
}

#[derive(Clone, derive_more::Deref, derive_more::DerefMut)]
pub struct OpStore {
    #[deref]
    #[deref_mut]
    pub(crate) store: SqliteStore,

    /// Decrypts encrypted operations for reading.
    streams: Arc<TopicStreams>,

    #[cfg(feature = "testing")]
    pub processed_ops: Arc<RwLock<HashMap<TopicId, HashSet<Hash>>>>,
}

impl OpStore {
    pub fn from_sqlite(store: SqliteStore, streams: Arc<TopicStreams>) -> Self {
        Self {
            store,
            streams,
            #[cfg(feature = "testing")]
            processed_ops: Arc::new(RwLock::new(HashMap::new())),
        }
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
        let cursor = p2panda_store::tx!(self.store, {
            CursorStore::<p2panda::VerifyingKey, LogId>::get_cursor_tx(
                &self.store,
                topic.to_string(),
            )
            .await?
        });
        Ok(cursor.and_then(|c| c.log_height(author, log_id).copied()))
    }

    /// Gracefully close the underlying SQLite pool (no-op for the in-memory variant).
    pub async fn close(&self) {
        self.store.pool().close().await;
    }

    /// Read stored operations, decrypting the payload of each encrypted one. p2panda stores
    /// such a payload only as ciphertext in the header.
    async fn read(&self, ops: Vec<Operation>) -> anyhow::Result<Vec<ReadOperation>> {
        let mut by_space: BTreeMap<TopicId, Vec<(usize, EncryptedMessage)>> = BTreeMap::new();
        for (index, op) in ops.iter().enumerate() {
            if let Some((space, message)) = encrypted_message(op) {
                by_space.entry(space).or_default().push((index, message));
            }
        }
        let mut read: Vec<ReadOperation> = ops
            .into_iter()
            .map(|op| ReadOperation {
                hash: op.hash,
                header: op.header,
                body: Ok(op.body),
            })
            .collect();
        for (space, messages) in by_space {
            let (indices, messages): (Vec<_>, Vec<_>) = messages.into_iter().unzip();
            let plaintexts = self.streams.decrypt_messages(space, messages).await?;
            for (index, plaintext) in indices.into_iter().zip(plaintexts) {
                read[index].body = plaintext.map(|plaintext| Some(plaintext.into()));
            }
        }
        Ok(read)
    }

    /// One author's log as the app reads it.
    pub async fn get_log(
        &self,
        author: &DeviceId,
        log_id: &LogId,
        from: Option<SeqNum>,
    ) -> anyhow::Result<Vec<ReadOperation>> {
        let log = self.log_operations(author, log_id, from).await?;
        if log.is_empty()
            && tracing::enabled!(tracing::Level::WARN)
            && !self.log_exists(author, log_id).await?
        {
            tracing::warn!(
                "No log found for log_id {} and author {}",
                Hash::from_bytes(*log_id.as_bytes()),
                author
            );
        }
        self.read(log).await
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

    /// The operations of a log from `from` that are safe to send onward, as
    /// they were stored. `None` means the log is absent.
    ///
    /// Only the contiguous prefix of fully-processed operations is returned.
    /// An operation that hasn't completed application-layer processing may
    /// still have its payload dropped by a tombstone it is about to
    /// enforce, so it must not be sent onward yet. Truncating (rather than
    /// filtering) keeps the returned log dense from `from`, which callers
    /// index by sequence number. Control operations (space, group and key
    /// bundle logs) are always safe to transmit — only application payloads
    /// are ever tombstoned — and are acknowledged by p2panda before ever
    /// reaching the application layer.
    async fn sendable_log_segment(
        &self,
        log: &Log,
        topic: &TopicId,
        from: SeqNum,
    ) -> anyhow::Result<Option<Vec<Operation>>> {
        let Log { author, log_id } = log;
        let log_id: LogId = (*log_id).into();
        let mut segment = self
            .log_operations(author, &log_id, from.checked_sub(1))
            .await?;
        if segment.is_empty() && !self.log_exists(author, &log_id).await? {
            return Ok(None);
        }
        let acked_height = self.acked_log_height(topic, author, &log_id).await?;
        let sendable = segment
            .iter()
            .take_while(|op| {
                !is_application(op)
                    || op.body.is_none()
                    || acked_height.is_some_and(|h| op.header.seq_num <= h)
            })
            .count();
        segment.truncate(sendable);
        Ok(Some(segment))
    }

    /// An operation exactly as it was signed and stored, safe to send onward.
    pub async fn get_stored_operation(&self, hash: &Hash) -> anyhow::Result<Option<Operation>> {
        use p2panda_store::operations::OperationStore;
        OperationStore::<Operation, Hash>::get_operation(&self.store, hash)
            .await
            .map_err(|err| anyhow::anyhow!("failed to get operation for {hash:?}: {err}"))
    }

    /// An operation as the app reads it.
    pub async fn get_operation(&self, hash: &Hash) -> anyhow::Result<Option<ReadOperation>> {
        let Some(operation) = self.get_stored_operation(hash).await? else {
            return Ok(None);
        };
        Ok(Some(self.read_one(operation).await?))
    }

    async fn read_one(&self, operation: Operation) -> anyhow::Result<ReadOperation> {
        let mut read = self.read(vec![operation]).await?;
        Ok(read.pop().expect("reading one operation gives one back"))
    }

    #[deprecated = "will be replace by proper use of p2panda-streams"]
    pub fn get_all_operations_not_fully_sorted(
        &self,
    ) -> impl futures::Stream<Item = Result<ReadOperation, anyhow::Error>> + '_ {
        queries::get_all_operations_not_fully_sorted(&self.store)
            .and_then(|operation| self.read_one(operation))
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
                match op.body {
                    Ok(Some(body)) => {
                        if let Ok(payload) = Payload::try_from_body(&body) {
                            logs.push((op.header, Some(payload)));
                        } else {
                            tracing::error!("Failed to decode payload: {body:?}");
                        }
                    }
                    Ok(None) => logs.push((op.header, None)),
                    // As far as a reader can tell, this operation has not arrived yet.
                    Err(Unreadable::SecretMissing) => {}
                    Err(err @ Unreadable::DecryptionFailed) => return Err(err.into()),
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
        p2panda_store::tx!(self.store, {
            OperationStore::<Operation, Hash>::delete_operation_payload_tx(&self.store, hash)
                .await?;
        });
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
        log: &Log,
        topic: &TopicId,
        from: SeqNum,
    ) -> Result<Option<Vec<MailboxOperation>>, anyhow::Error> {
        let Some(segment) = self.sendable_log_segment(log, topic, from).await? else {
            return Ok(None);
        };
        Ok(Some(
            segment
                .into_iter()
                .map(|op| MailboxOperation {
                    topic: *topic,
                    header: op.header,
                    body: op.body,
                })
                .collect(),
        ))
    }

    async fn get_log_blob_hashes(
        &self,
        log: &Log,
        topic: &TopicId,
        from: SeqNum,
    ) -> Result<Option<Vec<(SeqNum, Vec<iroh_blobs::Hash>)>>, anyhow::Error> {
        let Some(segment) = self.sendable_log_segment(log, topic, from).await? else {
            return Ok(None);
        };
        let mut blob_hashes = Vec::new();
        for op in self.read(segment).await? {
            let hashes = match &op.body {
                Ok(Some(body)) => crate::mailbox::media_blob_hashes(body.as_bytes()),
                Ok(None) | Err(Unreadable::DecryptionFailed) => Vec::new(),
                // Its blobs can be told once the secret arrives, so the walk must come back.
                Err(Unreadable::SecretMissing) => break,
            };
            blob_hashes.push((op.header.seq_num, hashes));
        }
        Ok(Some(blob_hashes))
    }

    async fn get_log_heights(&self, topic: &TopicId) -> anyhow::Result<Vec<(Log, SeqNum)>> {
        let logs: BTreeMap<p2panda::VerifyingKey, Vec<LogId>> = self.store.resolve(topic).await?;
        let mut heights = Vec::new();
        for (author, log_ids) in logs {
            let Some(log_heights) = self.store.get_log_heights(&author, &log_ids).await? else {
                continue;
            };
            for (log_id, height) in log_heights {
                heights.push((Log::new(DeviceId::from(author), log_id), height));
            }
        }
        Ok(heights)
    }
}

/// The space and ciphertext of an encrypted operation that has no body to read.
fn encrypted_message(op: &Operation) -> Option<(TopicId, EncryptedMessage)> {
    if op.body.is_some() {
        return None;
    }
    let ExtensionsVariantV1::Space(extensions) = &op.header.extensions.variant else {
        return None;
    };
    let SpacesArgs::Application {
        space_id,
        group_secret_id,
        nonce,
        ciphertext,
        ..
    } = &extensions.args
    else {
        return None;
    };
    Some((
        TopicId::from(*space_id),
        EncryptedMessage {
            group_secret_id: *group_secret_id,
            nonce: *nonce,
            ciphertext: ciphertext.clone(),
        },
    ))
}

/// Whether an operation carries an application payload, as opposed to a
/// space, group or key bundle control message.
fn is_application(op: &Operation) -> bool {
    match &op.header.extensions.variant {
        ExtensionsVariantV1::Basic(_) => true,
        ExtensionsVariantV1::Space(extensions) => {
            matches!(extensions.args, SpacesArgs::Application { .. })
        }
    }
}

#[cfg(test)]
mod tests {
    use mailbox_client::store::MailboxStore;
    use p2panda::operation::{Extensions, Header, LogId};
    use p2panda_core::{Body, Timestamp};
    use p2panda_store::Transaction;
    use p2panda_store::operations::OperationStore;

    use super::*;
    use crate::testing::*;

    /// Alice and bobbi as contacts who only reach each other through a mailbox.
    async fn contacts() -> (TestNode, TestNode, DirectChatId) {
        let mailbox = TestMailbox::from_env();
        let config = NodeConfig::testing().no_p2p();
        let alice = TestNode::new(config.clone(), "alice")
            .await
            .add_mailbox(&mailbox)
            .await;
        let bobbi = TestNode::new(config, "bobbi")
            .await
            .add_mailbox(&mailbox)
            .await;
        alice
            .behavior()
            .initiate_and_establish_contact(&bobbi)
            .await
            .unwrap();
        let chat = alice.direct_chat_with(&bobbi);
        (alice, bobbi, chat)
    }

    /// An operation of a plain stream topic, as sync stores it before it is processed.
    fn stream_op(
        signing_key: &p2panda::SigningKey,
        log_id: LogId,
        seq_num: SeqNum,
        backlink: Option<Hash>,
        payload: &Payload,
    ) -> Operation {
        let payload = payload.as_bytes().unwrap();
        // Timestamps track seq_num so the tests' expected ordering is deterministic.
        let extensions = Extensions::builder(log_id)
            .timestamp(Timestamp::new(seq_num.into()))
            .build();
        let header = Header::builder()
            .body(&payload)
            .seq_num(seq_num)
            .backlink(backlink)
            .build(signing_key, extensions);
        Operation {
            hash: header.hash(),
            header,
            body: Some(Body::from_bytes(&payload)),
        }
    }

    async fn insert(store: &OpStore, op: &Operation, log_id: &LogId) {
        let permit = store.store.begin().await.unwrap();
        OperationStore::<Operation, Hash>::insert_operation_tx(&store.store, &op.hash, op, log_id)
            .await
            .unwrap();
        store.store.commit(permit).await.unwrap();
    }

    /// Advance p2panda's ack cursor for `author`'s log to `seq`, mimicking what
    /// `ProcessedOperation::ack` persists once application-layer processing has
    /// finished (see `OpStore::acked_log_height`).
    async fn ack_up_to(
        store: &OpStore,
        topic: &TopicId,
        author: &DeviceId,
        log_id: LogId,
        seq: SeqNum,
    ) {
        use p2panda_core::Cursor;
        use p2panda_core::logs::LogHeights;
        use p2panda_store::cursors::CursorStore;

        let permit = store.store.begin().await.unwrap();
        let mut cursor = CursorStore::<p2panda::VerifyingKey, LogId>::get_cursor_tx(
            &store.store,
            topic.to_string(),
        )
        .await
        .unwrap()
        .unwrap_or_else(|| Cursor::new(topic.to_string(), LogHeights::default()));
        cursor.advance(**author, log_id, seq);
        CursorStore::set_cursor_tx(&store.store, &cursor)
            .await
            .unwrap();
        store.store.commit(permit).await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn mailbox_get_log_distinguishes_absent_from_up_to_date() {
        let (alice, _bobbi, chat) = contacts().await;
        let log_id = crate::topic::application_log_id(*chat);
        let stranger = DeviceId::from(p2panda::SigningKey::generate().verifying_key());

        for from in [0, 1] {
            let log =
                MailboxStore::get_log(&alice.op_store, &Log::new(stranger, log_id), &chat, from)
                    .await
                    .unwrap();
            assert!(log.is_none(), "absent log from {from}: {log:?}");
        }

        let message = alice.send_message(chat, "hello", None, None).await.unwrap();
        let alice_log = Log::new(alice.device_id(), log_id);
        let served = async |from: SeqNum| {
            MailboxStore::get_log(&alice.op_store, &alice_log, &chat, from)
                .await
                .unwrap()
                .map(|ops| {
                    ops.into_iter()
                        .map(|op| op.header.hash())
                        .collect::<Vec<_>>()
                })
        };
        assert_eq!(served(message.seq_num).await, Some(vec![message.hash()]));
        assert_eq!(served(message.seq_num + 1).await, Some(vec![]));
    }

    /// Mailbox sync must only see the contiguous prefix of a log whose
    /// operations have completed application-layer processing, as recorded by
    /// p2panda's ack cursor.
    #[tokio::test(flavor = "multi_thread")]
    async fn mailbox_get_log_truncates_at_first_unacked_op() {
        let node = TestNode::new(NodeConfig::testing().no_p2p(), "alice").await;
        let store = &node.op_store;
        let topic = TopicId::random();
        let log_id = LogId::from_topic(topic);
        let signing_key = p2panda::SigningKey::generate();
        let author = DeviceId::from(signing_key.verifying_key());
        let payload = Payload::DeviceGroup(DeviceGroupPayload::SetProfile(Profile {
            name: "Bobbi".into(),
            surname: None,
            avatar: None,
            about: None,
        }));

        let op0 = stream_op(&signing_key, log_id, 0, None, &payload);
        let op1 = stream_op(&signing_key, log_id, 1, Some(op0.hash), &payload);
        insert(store, &op0, &log_id).await;
        insert(store, &op1, &log_id).await;

        let served = async || {
            MailboxStore::get_log(store, &Log::new(author, log_id), &topic, 0)
                .await
                .unwrap()
                .unwrap()
                .into_iter()
                .map(|op| op.header.hash())
                .collect::<Vec<_>>()
        };

        // Nothing acked yet: no body-carrying op may be transmitted.
        assert_eq!(served().await, vec![]);

        ack_up_to(store, &topic, &author, log_id, 0).await;
        // Only op0 is acked; op1 sits above the watermark and truncates the log.
        assert_eq!(served().await, vec![op0.hash]);

        ack_up_to(store, &topic, &author, log_id, 1).await;
        assert_eq!(served().await, vec![op0.hash, op1.hash]);
    }

    /// The blobs of a media message are read out of its encrypted payload, by
    /// its author and by whoever received it.
    #[tokio::test(flavor = "multi_thread")]
    async fn mailbox_get_log_blob_hashes_reads_encrypted_messages() {
        let (alice, bobbi, chat) = contacts().await;
        let alice_log = Log::new(alice.device_id(), crate::topic::application_log_id(*chat));

        let photo = OutgoingMedia::Photos {
            photos: vec![OutgoingPhoto {
                data: rand::random::<[u8; 1024]>().to_vec(),
                name: "pic.png".into(),
                mime_type: "image/png".into(),
                width: 640,
                height: 480,
            }],
        };
        let message = alice
            .send_message(chat, "look at this", Some(photo), None)
            .await
            .unwrap();
        let media_hash = alice
            .get_messages(chat)
            .await
            .unwrap()
            .into_iter()
            .find_map(|m| m.content.media().cloned())
            .expect("the sent message has media")
            .first()
            .expect("one media item")
            .hash();
        let expected = (message.seq_num, vec![media_hash]);

        let blob_hashes = async |node: &TestNode| {
            node.op_store
                .get_log_blob_hashes(&alice_log, &chat, message.seq_num)
                .await
                .unwrap()
        };
        assert_eq!(blob_hashes(&alice).await, Some(vec![expected.clone()]));

        PollConfig::default()
            .wait_for(|| async {
                let read = blob_hashes(&bobbi).await;
                (read == Some(vec![expected.clone()]))
                    .then_some(())
                    .ok_or(read)
            })
            .await
            .unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "message deletion is temporarily disabled until p2panda moves encrypted payloads into the body"]
    async fn delete_body_drops_payload_keeps_header() {
        let (alice, _bobbi, chat) = contacts().await;
        let message = alice.send_message(chat, "hello", None, None).await.unwrap();
        let hash = message.hash();
        let stored = async || {
            alice
                .op_store
                .get_stored_operation(&hash)
                .await
                .unwrap()
                .unwrap()
        };
        assert!(stored().await.body.is_some());

        alice.op_store.delete_body(&hash).await.unwrap();

        let stored = stored().await;
        assert!(stored.body.is_none());
        // The header is retained so log sync stays consistent.
        assert_eq!(stored.header.seq_num, message.seq_num);
    }
}
