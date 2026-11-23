pub mod queries;

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::sync::Arc;
#[cfg(feature = "testing")]
use std::{collections::HashMap, sync::RwLock};

use dashchat_utils::SeqNum;
use futures::TryStreamExt;
use p2panda::Hash;
#[cfg(any(test, feature = "testing"))]
use p2panda::operation::Header;
use p2panda::operation::{ExtensionsVariantV1, LogId, Operation, SpacesHeaderArgs};
use p2panda_store::SqliteStore;
use p2panda_store::logs::LogStore;
use p2panda_store::topics::TopicStore;

use crate::node::streams::{EncryptedMessage, TopicStreams, Unreadable};
use crate::{mailbox::MailboxOperation, topic::TopicId, *};

/// The shape of one `(author, log)` in the op store: its lowest and highest
/// stored seq and how many rows lie in between.
#[cfg(any(test, feature = "lan-router"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LogSeqSummary {
    pub min: SeqNum,
    pub max: SeqNum,
    pub count: SeqNum,
}

#[cfg(any(test, feature = "lan-router"))]
impl LogSeqSummary {
    /// Every seq from `min` to `max` is present.
    pub fn is_contiguous(&self) -> bool {
        self.max - self.min + 1 == self.count
    }
}

/// The payload of an encrypted operation does not decrypt with the secret it names: the
/// ciphertext is corrupt, or was encrypted with something else.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("the payload does not decrypt with the secret it names")]
pub struct Undecryptable;

/// An operation as the app reads it, see [`OpStore::get_log`]. Never send one onward: mailbox
/// sync reads the stored operations instead.
///
/// The app only ever sees the default `E`, which cannot express a missing secret. Within the
/// crate, `ReadOperation<Unreadable>` keeps that case for callers that must wait for it.
//
// TODO: once we have a projection of valid, processed operations that caches their plaintext,
// nothing reads an operation whose secret is missing and `E` can go.
#[derive(Clone, Debug)]
pub struct ReadOperation<E = Undecryptable> {
    pub hash: Hash,
    pub header: p2panda::operation::Header,
    /// The payload, decrypted if the operation was published into a space. `Ok(None)` is an
    /// operation that carries none: a control message, or one whose payload was deleted.
    pub body: Result<Option<p2panda_core::Body>, E>,
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

    /// [`Self::acked_log_height`] for every author of `log_id` in `topic`,
    /// from one read of the topic's cursor.
    #[cfg(any(test, feature = "lan-router"))]
    pub(crate) async fn acked_log_heights(
        &self,
        topic: &TopicId,
        log_id: &LogId,
    ) -> anyhow::Result<BTreeMap<DeviceId, SeqNum>> {
        use p2panda_store::cursors::CursorStore;
        let cursor = p2panda_store::tx!(self.store, {
            CursorStore::<p2panda::VerifyingKey, LogId>::get_cursor_tx(
                &self.store,
                topic.to_string(),
            )
            .await?
        });
        let Some(cursor) = cursor else {
            return Ok(BTreeMap::new());
        };
        Ok(cursor
            .state()
            .iter()
            .filter_map(|(author, logs)| Some((DeviceId::from(*author), *logs.get(log_id)?)))
            .collect())
    }

    /// Changes whenever an operation is stored, by whichever path: p2panda
    /// stores control operations that no stream event or ack announces.
    #[cfg(any(test, feature = "lan-router"))]
    pub(crate) async fn latest_operation_rowid(&self) -> anyhow::Result<Option<i64>> {
        queries::latest_operation_rowid(&self.store).await
    }

    /// The logs p2panda has associated with `topic`, by author.
    #[cfg(any(test, feature = "lan-router"))]
    pub(crate) async fn topic_logs(
        &self,
        topic: &TopicId,
    ) -> anyhow::Result<BTreeMap<DeviceId, Vec<LogId>>> {
        let logs: BTreeMap<p2panda::VerifyingKey, Vec<LogId>> = self.store.resolve(topic).await?;
        Ok(logs
            .into_iter()
            .map(|(author, logs)| (DeviceId::from(author), logs))
            .collect())
    }

    /// The log of every group that `authors`' stored control operations of
    /// the space `topic` name. Read from the operations as stored, so a
    /// group is found before the space can be processed.
    #[cfg(any(test, feature = "lan-router"))]
    pub(crate) async fn space_group_log_ids(
        &self,
        topic: &TopicId,
        authors: impl IntoIterator<Item = &DeviceId>,
    ) -> anyhow::Result<BTreeSet<LogId>> {
        let space_log = p2panda::spaces::space_log_id((*topic).into());
        let mut group_logs = BTreeSet::new();
        for author in authors {
            let ops = self.log_operations(author, &space_log, None, None).await?;
            group_logs.extend(ops.iter().filter_map(space_group_log_id));
        }
        Ok(group_logs)
    }

    /// Every sequence number present for `author`'s log, ascending. The
    /// router's held-range input when a log has gaps (see `lan_router.rs`).
    #[cfg(any(test, feature = "lan-router"))]
    pub(crate) async fn get_log_seqs(
        &self,
        author: &DeviceId,
        log_id: &LogId,
    ) -> anyhow::Result<Vec<SeqNum>> {
        queries::get_log_seqs(&self.store, author, log_id).await
    }

    /// One [`LogSeqSummary`] per author of `log_id`, from a single query.
    #[cfg(any(test, feature = "lan-router"))]
    pub(crate) async fn get_log_seq_summaries(
        &self,
        log_id: &LogId,
    ) -> anyhow::Result<BTreeMap<DeviceId, LogSeqSummary>> {
        queries::get_log_seq_summaries(&self.store, log_id, None).await
    }

    #[cfg(any(test, feature = "lan-router"))]
    pub(crate) async fn get_log_seq_summary(
        &self,
        author: &DeviceId,
        log_id: &LogId,
    ) -> anyhow::Result<Option<LogSeqSummary>> {
        let mut summaries =
            queries::get_log_seq_summaries(&self.store, log_id, Some(author)).await?;
        Ok(summaries.remove(author))
    }

    /// Gracefully close the underlying SQLite pool (no-op for the in-memory variant).
    pub async fn close(&self) {
        self.store.pool().close().await;
    }

    /// Stored operations as the app reads them.
    ///
    /// An encrypted operation whose secret has not arrived is left out. p2panda stores it on
    /// arrival but only hands it over for processing once it decrypts, so until then the app
    /// has not received it.
    //
    // TODO: every other stored operation is returned, whether or not it has been processed
    // or has passed validation. Reading should move to a projection of valid, processed
    // operations that caches their plaintext, which replaces decrypting here on every read.
    async fn read(&self, ops: Vec<Operation>) -> anyhow::Result<Vec<ReadOperation>> {
        let read = self.decrypt(ops).await?.into_iter().filter_map(|op| {
            let body = match op.body {
                Ok(body) => Ok(body),
                Err(Unreadable::SecretMissing) => return None,
                Err(Unreadable::DecryptionFailed) => Err(Undecryptable),
            };
            Some(ReadOperation {
                hash: op.hash,
                header: op.header,
                body,
            })
        });
        Ok(read.collect())
    }

    /// Decrypt the payload of each encrypted operation.
    //
    // TODO: once we have a projection of valid, processed operations that caches their plaintext,
    // we can afford to decrypt one message at a time instead of in a batch, which simplifies
    // the logic and types.
    async fn decrypt(&self, ops: Vec<Operation>) -> anyhow::Result<Vec<ReadOperation<Unreadable>>> {
        let mut by_space: BTreeMap<TopicId, Vec<(usize, EncryptedMessage)>> = BTreeMap::new();
        for (index, op) in ops.iter().enumerate() {
            if let Some((space, message)) = encrypted_message(op) {
                by_space.entry(space).or_default().push((index, message));
            }
        }
        let mut read: Vec<ReadOperation<Unreadable>> = ops
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
        let log = self.log_operations(author, log_id, from, None).await?;
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

    /// `author`'s log after `from` (exclusive) up to `until` (inclusive), each
    /// open-ended when `None`.
    #[cfg(any(test, feature = "lan-router"))]
    pub(crate) async fn get_log_until(
        &self,
        author: &DeviceId,
        log_id: &LogId,
        from: Option<SeqNum>,
        until: Option<SeqNum>,
    ) -> anyhow::Result<Vec<Operation>> {
        self.log_operations(author, log_id, from, until).await
    }

    /// Collect a log's entries, decoding each stored operation into our extension type.
    async fn log_operations(
        &self,
        author: &DeviceId,
        log_id: &LogId,
        from: Option<SeqNum>,
        until: Option<SeqNum>,
    ) -> anyhow::Result<Vec<Operation>> {
        self.store
            .log_entries(author, log_id, from, until)?
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
            .log_operations(author, &log_id, from.checked_sub(1), None)
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

    /// Whether an operation is stored, with or without its body.
    #[cfg(feature = "lan-router")]
    pub(crate) async fn has_operation(&self, hash: &Hash) -> anyhow::Result<bool> {
        use p2panda_store::operations::OperationStore;
        Ok(p2panda_store::tx!(self.store, {
            OperationStore::<Operation, Hash>::has_operation_tx(&self.store, hash).await?
        }))
    }

    /// An operation as the app reads it.
    pub async fn get_operation(&self, hash: &Hash) -> anyhow::Result<Option<ReadOperation>> {
        let Some(operation) = self.get_stored_operation(hash).await? else {
            return Ok(None);
        };
        Ok(self.read(vec![operation]).await?.pop())
    }

    #[deprecated = "will be replace by proper use of p2panda-streams"]
    pub fn get_all_operations_not_fully_sorted(
        &self,
    ) -> impl futures::Stream<Item = Result<ReadOperation, anyhow::Error>> + '_ {
        queries::get_all_operations_not_fully_sorted(&self.store)
            .try_filter_map(async |operation| Ok(self.read(vec![operation]).await?.pop()))
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
                    Err(err) => return Err(err.into()),
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
        for op in self.decrypt(segment).await? {
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

/// The space and ciphertext of an encrypted operation whose payload has not been deleted.
fn encrypted_message(op: &Operation) -> Option<(TopicId, EncryptedMessage)> {
    let ExtensionsVariantV1::Space(extensions) = &op.header.extensions.variant else {
        return None;
    };
    let SpacesHeaderArgs::Application {
        space_id,
        group_secret_id,
        nonce,
        ..
    } = &extensions.args
    else {
        return None;
    };
    let ciphertext = op.body.as_ref()?.to_bytes();
    Some((
        TopicId::from(*space_id),
        EncryptedMessage {
            group_secret_id: *group_secret_id,
            nonce: *nonce,
            ciphertext,
        },
    ))
}

/// The log of the group a space control operation names.
#[cfg(any(test, feature = "lan-router"))]
fn space_group_log_id(op: &Operation) -> Option<LogId> {
    let ExtensionsVariantV1::Space(extensions) = &op.header.extensions.variant else {
        return None;
    };
    match &extensions.args {
        SpacesHeaderArgs::KeyAgreement { group_id, .. }
        | SpacesHeaderArgs::SpaceUpdate { group_id, .. } => {
            Some(p2panda::spaces::group_log_id(*group_id))
        }
        _ => None,
    }
}

/// Whether an operation carries an application payload, as opposed to a
/// space, group or key bundle control message.
fn is_application(op: &Operation) -> bool {
    match &op.header.extensions.variant {
        ExtensionsVariantV1::Basic(_) => true,
        ExtensionsVariantV1::Space(extensions) => {
            matches!(extensions.args, SpacesHeaderArgs::Application { .. })
        }
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use p2panda::operation::{Extensions, Header};
    use p2panda_core::{Body, Timestamp};
    use p2panda_store::Transaction;
    use p2panda_store::operations::OperationStore;

    use super::*;

    /// An operation of a plain stream topic, as sync stores it before it is processed.
    pub(crate) fn signed_op(
        signing_key: &p2panda::SigningKey,
        log_id: LogId,
        seq_num: SeqNum,
        backlink: Option<Hash>,
        payload: &[u8],
    ) -> Operation {
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
            body: Some(Body::from_bytes(payload)),
        }
    }

    pub(crate) async fn insert(store: &OpStore, op: &Operation, log_id: &LogId) {
        let permit = store.store.begin().await.unwrap();
        OperationStore::<Operation, Hash>::insert_operation_tx(&store.store, &op.hash, op, log_id)
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
}

#[cfg(test)]
mod tests {
    use mailbox_client::store::MailboxStore;
    use maplit::btreemap;
    use p2panda::operation::{Header, LogId};
    use p2panda_core::Body;

    use super::test_support::*;
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

    fn stream_op(
        signing_key: &p2panda::SigningKey,
        log_id: LogId,
        seq_num: SeqNum,
        backlink: Option<Hash>,
        payload: &Payload,
    ) -> Operation {
        signed_op(
            signing_key,
            log_id,
            seq_num,
            backlink,
            &payload.as_bytes().unwrap(),
        )
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

    /// A chat message stored by a node that holds none of the chat's secrets is left out of
    /// what the app reads, though the store has it.
    #[tokio::test(flavor = "multi_thread")]
    async fn operation_without_its_secret_is_not_read() {
        let (alice, _bobbi, chat) = contacts().await;
        let carol = TestNode::new(NodeConfig::testing().no_p2p(), "carol").await;
        let log_id = crate::topic::application_log_id(*chat);

        let message = alice.send_message(chat, "hello", None, None).await.unwrap();
        let hash = message.hash();
        let op = alice
            .op_store
            .get_stored_operation(&hash)
            .await
            .unwrap()
            .unwrap();
        insert(&carol.op_store, &op, &log_id).await;

        assert!(
            carol
                .op_store
                .get_stored_operation(&hash)
                .await
                .unwrap()
                .is_some()
        );
        assert!(carol.op_store.get_operation(&hash).await.unwrap().is_none());
        let log = carol
            .op_store
            .get_log(&alice.device_id(), &log_id, None)
            .await
            .unwrap();
        assert!(log.is_empty(), "{log:?}");
    }

    /// A chat message whose ciphertext was tampered with reads as undecryptable rather than
    /// being dropped or failing the read.
    #[tokio::test(flavor = "multi_thread")]
    async fn tampered_ciphertext_reads_as_undecryptable() {
        let (alice, _bobbi, chat) = contacts().await;
        let log_id = crate::topic::application_log_id(*chat);

        let message = alice.send_message(chat, "hello", None, None).await.unwrap();
        let original = alice
            .op_store
            .get_stored_operation(&message.hash())
            .await
            .unwrap()
            .unwrap();
        let mut ciphertext = original.body.expect("a chat message has a body").to_bytes();
        ciphertext[0] ^= 1;
        let header = Header::builder().body(&ciphertext).build(
            &p2panda::SigningKey::generate(),
            original.header.extensions.clone(),
        );
        let tampered = Operation {
            hash: header.hash(),
            header,
            body: Some(Body::from_bytes(&ciphertext)),
        };
        insert(&alice.op_store, &tampered, &log_id).await;

        let read = alice
            .op_store
            .get_operation(&tampered.hash)
            .await
            .unwrap()
            .expect("the tampered operation is read");
        assert_eq!(read.body.unwrap_err(), Undecryptable);
    }

    #[tokio::test(flavor = "multi_thread")]
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

    #[tokio::test(flavor = "multi_thread")]
    async fn get_log_seqs_lists_present_seqs_ascending() {
        let node = TestNode::new(NodeConfig::testing().no_p2p(), "alice").await;
        let store = &node.op_store;
        let key = p2panda::SigningKey::generate();
        let author = DeviceId::from(key.verifying_key());
        let log_id = LogId::from_topic(TopicId::random());
        let op0 = signed_op(&key, log_id, 0, None, b"a");
        let op1 = signed_op(&key, log_id, 1, Some(op0.hash), b"b");
        let op2 = signed_op(&key, log_id, 2, Some(op1.hash), b"c");
        insert(store, &op2, &log_id).await;
        insert(store, &op0, &log_id).await;
        insert(store, &op1, &log_id).await;
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

    /// Chained ops `0..n` for `key`, not inserted.
    fn chain(key: &p2panda::SigningKey, log_id: LogId, n: SeqNum) -> Vec<Operation> {
        let mut ops = Vec::new();
        let mut backlink = None;
        for seq in 0..n {
            let op = signed_op(key, log_id, seq, backlink, &[seq as u8]);
            backlink = Some(op.hash);
            ops.push(op);
        }
        ops
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn log_seq_summaries_describe_a_run_and_a_gap() {
        let node = TestNode::new(NodeConfig::testing().no_p2p(), "alice").await;
        let store = &node.op_store;
        let log_id = LogId::from_topic(TopicId::random());
        let run_key = p2panda::SigningKey::generate();
        let gap_key = p2panda::SigningKey::generate();
        for op in chain(&run_key, log_id, 3) {
            insert(store, &op, &log_id).await;
        }
        for op in chain(&gap_key, log_id, 6)
            .iter()
            .filter(|op| [2, 3, 5].contains(&op.header.seq_num))
        {
            insert(store, op, &log_id).await;
        }
        let run = DeviceId::from(run_key.verifying_key());
        let gap = DeviceId::from(gap_key.verifying_key());
        let run_summary = LogSeqSummary {
            min: 0,
            max: 2,
            count: 3,
        };
        let gap_summary = LogSeqSummary {
            min: 2,
            max: 5,
            count: 3,
        };
        assert!(run_summary.is_contiguous());
        assert!(!gap_summary.is_contiguous());
        assert_eq!(
            store.get_log_seq_summaries(&log_id).await.unwrap(),
            btreemap! { run => run_summary, gap => gap_summary }
        );
        assert_eq!(
            store.get_log_seq_summary(&gap, &log_id).await.unwrap(),
            Some(gap_summary)
        );
        let other = DeviceId::from(p2panda::SigningKey::generate().verifying_key());
        assert_eq!(
            store.get_log_seq_summary(&other, &log_id).await.unwrap(),
            None
        );
        let other_log = LogId::from_topic(TopicId::random());
        assert!(
            store
                .get_log_seq_summaries(&other_log)
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn acked_log_heights_lists_every_author_of_the_log() {
        let node = TestNode::new(NodeConfig::testing().no_p2p(), "alice").await;
        let store = &node.op_store;
        let topic = TopicId::random();
        let log_id = LogId::from_topic(topic);
        let a = DeviceId::from(p2panda::SigningKey::generate().verifying_key());
        let b = DeviceId::from(p2panda::SigningKey::generate().verifying_key());
        assert!(
            store
                .acked_log_heights(&topic, &log_id)
                .await
                .unwrap()
                .is_empty()
        );
        ack_up_to(store, &topic, &a, log_id, 1).await;
        ack_up_to(store, &topic, &b, log_id, 3).await;
        ack_up_to(store, &topic, &b, LogId::from_topic(TopicId::random()), 9).await;
        assert_eq!(
            store.acked_log_heights(&topic, &log_id).await.unwrap(),
            btreemap! { a => 1, b => 3 }
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn get_log_until_bounds_both_ends() {
        let node = TestNode::new(NodeConfig::testing().no_p2p(), "alice").await;
        let store = &node.op_store;
        let key = p2panda::SigningKey::generate();
        let log_id = LogId::from_topic(TopicId::random());
        for op in chain(&key, log_id, 5) {
            insert(store, &op, &log_id).await;
        }
        let author = DeviceId::from(key.verifying_key());
        let seqs = |ops: Vec<Operation>| ops.iter().map(|op| op.header.seq_num).collect::<Vec<_>>();
        assert_eq!(
            seqs(
                store
                    .get_log_until(&author, &log_id, Some(1), Some(3))
                    .await
                    .unwrap()
            ),
            vec![2, 3]
        );
        assert_eq!(
            seqs(
                store
                    .get_log_until(&author, &log_id, None, Some(1))
                    .await
                    .unwrap()
            ),
            vec![0, 1]
        );
        assert_eq!(
            seqs(
                store
                    .get_log_until(&author, &log_id, None, Some(99))
                    .await
                    .unwrap()
            ),
            vec![0, 1, 2, 3, 4]
        );
    }
}
