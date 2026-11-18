use anyhow::anyhow;
use derive_more::derive::From;
use futures::StreamExt;
use p2panda::NodeId;
use p2panda::operation::Header;
use p2panda::streams::{ProcessedOperation, Source};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::error::TrySendError;
use tokio_stream::wrappers::ReceiverStream;
use tracing::{debug, error, warn};

use crate::AckedOp;
use crate::Log;
use crate::forward_edit_closure;
use crate::node::publish::warn_if_slow;
use crate::node::streams::{ProcessedTx, ProcessorError, ProcessorEvent, Space, TopicDrain};
use crate::stores::{BadUseOfNode, ProjectionError, TombstoneReason};
use crate::topic::AutoRegisteredTopic;

use super::*;

#[derive(Clone, Debug, Serialize, Deserialize, From)]
pub enum Notification {
    Op(OpNotification),
    System(SystemNotification),
}

impl Notification {
    pub fn op(&self) -> Option<&OpNotification> {
        if let Notification::Op(operation) = self {
            Some(operation)
        } else {
            None
        }
    }

    pub fn system(&self) -> Option<&SystemNotification> {
        if let Notification::System(system) = self {
            Some(system)
        } else {
            None
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OpNotification {
    pub topic: TopicId,
    #[serde(with = "crate::header_serde")]
    pub header: Header,
    pub payload: Option<Payload>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum SystemNotification {
    /// A group chat was added: its creator introduced the initial members.
    GroupChatAdded { chat_id: ChatId },
    /// Members of a group chat were added, removed, promoted or demoted.
    GroupMembersChanged { chat_id: ChatId },
    /// A group-auth op on `topic`'s space. It lives on the group log, not the
    /// chat log the frontend reads, so it is never announced as an operation.
    Membership {
        topic: TopicId,
        #[serde(with = "crate::header_serde")]
        header: Header,
    },
    /// Devices were introduced as belonging to agents.
    AgentsIntroduced { agents: BTreeMap<DeviceId, AgentId> },
    /// A device published a newer profile. `agent_id` is absent while the
    /// device's agent is still unknown.
    ProfileChanged {
        device_id: DeviceId,
        agent_id: Option<AgentId>,
    },
    /// A new tombstone has been created.
    Tombstones {
        topic: TopicId,
        hashes: BTreeSet<Hash>,
        reason: TombstoneReason,
    },
    /// New delivery acknowledgements were recorded for a chat topic. `acks`
    /// carries only the entries that changed, already filtered to ackers of a
    /// different agent than the author, so consumers can fold it into their
    /// delivered state (max seq per author) without re-querying.
    MessageAcks {
        topic: TopicId,
        acks: BTreeMap<DeviceId, AckedOp>,
    },
}

impl Node {
    /// Register a topic as subscribed in the database, and initialize it.
    /// When the node restarts, the topic will be rejoined as a space by
    /// [`Self::initialize_space`]: every auto-registered topic is a space.
    ///
    /// Note that some topics are excluded from automatic registration, such as inbox topics.
    /// They have to be registered separately with extra context.
    ///
    /// Idempotent.
    #[tracing::instrument(skip(self), fields(topic = ?topic.aliased()))]
    pub(crate) async fn register_and_join_space<K: AutoRegisteredTopic>(
        &self,
        topic: Topic<K>,
    ) -> anyhow::Result<()> {
        self.local_store.register_space_as_subscribed(topic).await?;
        self.initialize_space(*topic, None).await?;
        Ok(())
    }

    /// Create a space we own, persist it for restart, and import its mailbox stream.
    pub(crate) async fn create_and_register_space(&self) -> anyhow::Result<(ChatId, Space)> {
        let (chat_id, space) = self.streams.create_space().await?;
        self.register_created_space(chat_id, space).await
    }

    #[cfg(feature = "testing")]
    pub(crate) async fn create_and_register_space_at(
        &self,
        chat_id: ChatId,
    ) -> anyhow::Result<(ChatId, Space)> {
        let (_, space) = self.streams.create_space_at(*chat_id).await?;
        self.register_created_space(chat_id, space).await
    }

    async fn register_created_space(
        &self,
        chat_id: ChatId,
        space: Space,
    ) -> anyhow::Result<(ChatId, Space)> {
        let topic: TopicId = chat_id.into();
        self.local_store
            .register_space_as_subscribed(chat_id)
            .await?;
        self.import_mailbox_stream(topic).await?;
        self.notify_topic_subscribed(topic).await;
        Ok((chat_id, space))
    }

    /// The device whose contact code advertises the inbox `topic`: us, or
    /// someone whose code we scanned. An inbox topic is derived from its
    /// owner's device id, so whoever holds the code knows who it is for.
    async fn inbox_owner(&self, topic: TopicId) -> anyhow::Result<Option<DeviceId>> {
        let advertised = self.local_store.get_advertised_inbox_topics().await?;
        if advertised.iter().any(|inbox| *inbox.topic == topic) {
            return Ok(Some(self.device_id()));
        }
        let requested = self
            .local_store
            .get_requested_inbox_topics_with_owner()
            .await?;
        Ok(requested
            .into_iter()
            .find(|(inbox, _)| *inbox.topic == topic)
            .map(|(_, owner)| owner))
    }

    pub(crate) async fn join_presence(&self, chat_id: ChatId) -> anyhow::Result<()> {
        if !self.config.enable_p2p {
            return Ok(());
        }
        let stream = self.streams.ephemeral_stream((*chat_id).into()).await?;
        self.online_peers.join(*chat_id, stream).await;
        Ok(())
    }

    /// Internal function to start the necessary tasks for processing network activity
    /// for a given topic.
    ///
    /// This must be called:
    /// - when creating a new group chat
    /// - when initializing the node, for each existing group chat
    #[tracing::instrument(skip(self), fields(topic = ?topic.aliased()))]
    pub(crate) async fn initialize_stream(&self, topic: TopicId) -> anyhow::Result<()> {
        topic.alias_numbered();

        tracing::info!(topic = ?topic.aliased(), "initialize topic");

        self.subscribe_to_stream(topic).await?;
        self.import_mailbox_stream(topic).await?;
        Ok(())
    }

    /// Rejoin a registered space on node startup and import its mailbox stream.
    #[tracing::instrument(skip(self), fields(topic = ?topic.aliased()))]
    ///
    /// `accepted` spares looking the accepted contacts up again for a caller
    /// that initializes many spaces.
    pub(crate) async fn initialize_space(
        &self,
        topic: TopicId,
        accepted: Option<&BTreeSet<AgentId>>,
    ) -> anyhow::Result<()> {
        topic.alias_numbered();
        tracing::info!(topic = ?topic.aliased(), "initialize space");

        self.streams.join_space(topic).await?;
        self.import_mailbox_stream(topic).await?;
        if self.wakes_device(topic, accepted).await? {
            self.notify_topic_subscribed(topic).await;
        }
        Ok(())
    }

    /// Whether traffic in a space is worth waking the device for. It is not in
    /// a space someone made by requesting contact, until we accept them:
    /// anyone holding our code could otherwise wake us at will.
    async fn wakes_device(
        &self,
        topic: TopicId,
        accepted: Option<&BTreeSet<AgentId>>,
    ) -> anyhow::Result<bool> {
        let chat_id = ChatId::from_topic_id(topic);
        let Some(peer) = self.projection.direct_chat_peer(chat_id).await? else {
            return Ok(true);
        };
        if self.requested_direct_chat_id(peer).await? == Some(chat_id) {
            return Ok(true);
        }
        let Some(agent_id) = self.lookup_contact(peer).await? else {
            return Ok(false);
        };
        Ok(match accepted {
            Some(accepted) => accepted.contains(&agent_id),
            None => self.accepted_contact_agent_ids().await?.contains(&agent_id),
        })
    }

    /// Stop following a space: it is not rejoined on restart and nothing more
    /// of it is fetched. The inverse of [`Self::register_and_join_space`].
    pub(crate) async fn leave_space(&self, chat_id: ChatId) -> anyhow::Result<()> {
        self.local_store
            .register_topic_as_unsubscribed(chat_id)
            .await?;
        self.online_peers.leave(*chat_id).await;
        self.mailboxes.unsubscribe(chat_id.into()).await?;
        self.streams.unsubscribe(*chat_id).await;
        Ok(())
    }

    /// Subscribe to a non-Space topic stream.
    async fn subscribe_to_stream(&self, topic: TopicId) -> anyhow::Result<()> {
        debug!(topic = ?topic.aliased(), "subscribe to stream");

        self.streams.subscribe(topic.into()).await?;
        self.notify_topic_subscribed(topic).await;
        Ok(())
    }

    /// Tell the host (push notifications) that this node now receives traffic for a topic.
    pub(super) async fn notify_topic_subscribed(&self, topic: TopicId) {
        if let Some(tx) = &self.topic_subscribed_tx {
            let _ = tx.send(topic).await;
        }
    }

    /// Import external operation stream from a mailbox, unless it is already imported.
    pub(super) async fn import_mailbox_stream(&self, topic: TopicId) -> anyhow::Result<()> {
        let Some(mailbox_rx) = self.mailboxes.subscribe(topic.into()).await? else {
            return Ok(());
        };
        debug!(topic = ?topic.aliased(), "import mailbox stream");

        let stream = Box::pin(ReceiverStream::new(mailbox_rx).map(Operation::from));
        self.import_stream(topic, stream).await?;

        // What the topic already holds is waiting in the mailbox: fetch it
        // now rather than at the next scheduled poll.
        self.mailboxes.request_sync(Some(topic.into())).await;
        Ok(())
    }

    pub(super) async fn import_stream(
        &self,
        topic: TopicId,
        stream: std::pin::Pin<Box<dyn futures::Stream<Item = Operation> + Send>>,
    ) -> anyhow::Result<()> {
        self.streams.import(topic.into(), stream).await?;
        Ok(())
    }

    /// Spawn a task for application layer processing of received operations.
    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all, fields(me=?self.device_id().aliased())))]
    pub(super) fn spawn_application_processor_task(
        &self,
        mut drain: TopicDrain,
        mut cancel_rx: mpsc::Receiver<()>,
    ) -> tokio::task::JoinHandle<()> {
        let node = self.clone();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    event = drain.next() => match event {
                        Some(ProcessorEvent::Operation { operation, source, processed_tx }) => {
                            // A stalled processor stalls p2panda's bounded pipeline, so this
                            // is the only early warning that it has fallen behind.
                            warn_if_slow(
                                "processing an operation",
                                node.process_operation(operation, source, processed_tx),
                            )
                            .await;
                        }
                        Some(ProcessorEvent::GroupMembershipOp { topic, header }) => {
                            node.notify(SystemNotification::Membership { topic: topic.into(), header }.into());
                        }
                        Some(ProcessorEvent::SpaceMembersChanged { topic }) => {
                            if let Err(err) = node.notify_group_members_changed(topic).await {
                                warn!(topic = ?topic.aliased(), ?err, "failed to announce a membership change");
                            }
                        }
                        Some(ProcessorEvent::ImportFailed { topic, error }) => {
                            error!(topic = ?topic.aliased(), ?error, "import failed; unsubscribing topic from mailbox so it can be re-imported on retry");
                            if let Err(err) = node.mailboxes.unsubscribe(topic).await {
                                error!(topic = ?topic.aliased(), ?err, "failed to unsubscribe topic after import failure");
                            }
                        }
                        None => break,
                    },
                    Some(()) = cancel_rx.recv() => {
                        tracing::info!("stream processing loop cancelled");
                        break;
                    }
                }
            }

            tracing::info!("stream processing loop finished");
        })
    }

    /// Process one received operation, resolve its `processed_tx` with the result, and acknowledge
    /// it if processing succeeded so that a failed operation is replayed another time.
    async fn process_operation(
        &self,
        operation: ProcessedOperation<Payload>,
        source: Source,
        processed_tx: Option<ProcessedTx>,
    ) {
        tracing::info!(op = ?operation.id().aliased(), topic = ?operation.topic().aliased(), "operation processing");

        let result = if self.recorded_for_app(&operation).await {
            self.process_app(&operation, &source)
                .await
                .map_err(|err| ProcessorError::App(err.to_string()))
        } else {
            Err(ProcessorError::System(
                "failed to record the operation for the app".to_string(),
            ))
        };

        if let Some(processed_tx) = processed_tx
            && processed_tx
                .send(
                    result
                        .clone()
                        .map(|()| operation.processed().header().clone()),
                )
                .is_err()
        {
            error!("processed_tx send error");
        }

        if let Err(err) = result {
            error!(?err, "process operation error");
            return;
        }

        if let Err(err) = self.ack_operation(&operation).await {
            error!(?err, "failed to acknowledge operation");
        }
    }

    /// As the push extension, record `operation` for the app to process too,
    /// unless the app acknowledged it already or authored it. Done before
    /// processing it, so the record outlives the extension being killed
    /// mid-way; returns whether to go on processing, since acknowledging an
    /// unrecorded operation would hide it from the app for good.
    async fn recorded_for_app(&self, operation: &ProcessedOperation<Payload>) -> bool {
        if !self.config.record_processed_operations {
            return true;
        }
        let author = DeviceId::from(operation.author());
        if author == self.device_id() {
            return true;
        }
        let topic = TopicId::from(operation.topic());
        let header = operation.processed().header();
        let recorded = async {
            let acked = self
                .op_store
                .acked_log_height(&topic, &author, &header.extensions.log_id())
                .await?;
            if acked.is_some_and(|acked| acked >= header.seq_num) {
                return anyhow::Ok(());
            }
            self.local_store
                .record_extension_processed_operation(operation.id(), topic)
                .await
        }
        .await;
        match recorded {
            Ok(()) => true,
            Err(err) => {
                error!(
                    ?err,
                    "failed to record an operation for the app to process; leaving it unprocessed"
                );
                false
            }
        }
    }

    async fn ack_operation(&self, operation: &ProcessedOperation<Payload>) -> anyhow::Result<()> {
        // Mark the operation as processed so it can be awaited by
        // [`crate::testing::PollConfig::consistency`]
        #[cfg(feature = "testing")]
        self.op_store
            .mark_op_processed(operation.topic(), &operation.id());

        // Acknowledge the operation now that application-layer
        // processing has finished. The node uses an `Explicit`
        // ack policy, so this persisted ack is what makes the
        // operation eligible for mailbox transmission (see
        // `OpStore::acked_log_height`).
        operation.ack().await?;
        if self.config.import_recorded_operations
            && let Err(err) = self
                .local_store
                .forget_extension_processed_operation(&operation.id())
                .await
        {
            // At worst the operation is imported once more.
            warn!(
                ?err,
                "failed to forget an operation the push extension recorded"
            );
        }

        if DeviceId::from(operation.author()) == self.device_id() {
            let log_id = operation.processed().header().extensions.log_id();
            let log = Log::new(self.device_id(), log_id);
            self.mailboxes
                .publish_fast_push(operation.topic(), log)
                .await;
        }

        Ok(())
    }

    /// Enforce the topic's tombstone set on a received operation: if its hash
    /// has been tombstoned, drop its stored payload so it is never persisted or
    /// synced onward. The operation arrives here already written to the op
    /// store (by peers or by mailbox sync), so this deletes the body after the
    /// fact.
    //
    // TODO: when device groups exist, test that tombstones are enforced across devices.
    async fn enforce_tombstone(&self, topic: TopicId, hash: Hash) -> anyhow::Result<bool> {
        if !self.projection.is_tombstoned(topic, hash).await? {
            return Ok(false);
        }
        let Some(tombstoned_op) = self.op_store.get_operation(&hash).await? else {
            return Ok(false);
        };
        self.unprocess_app(&tombstoned_op).await?;
        self.op_store.delete_body(&hash).await?;
        Ok(true)
    }

    /// Filter out operations whose payloads are not able to be deleted.
    pub(crate) fn is_tombstoneable(payload: &Payload) -> bool {
        matches!(
            payload,
            Payload::Chat(ChatPayload::Message(_) | ChatPayload::EditMessage { .. })
        )
    }

    /// Undo the effects of processing an app-layer operation,
    /// reverting to a state as if the operation were never processed.
    /// This is done when an operation payload is deleted.
    /// Only tombstoneable operations can be unprocessed.
    pub(crate) async fn unprocess_app(&self, operation: &ReadOperation) -> anyhow::Result<()> {
        // An operation we can not read was never processed.
        let Ok(Some(body)) = &operation.body else {
            return Ok(());
        };
        let payload = Payload::try_from_body(body)?;
        if Self::is_tombstoneable(&payload) {
            match payload {
                Payload::Chat(ChatPayload::Message(m)) => {
                    let author = operation.header.verifying_key;
                    let log_id = operation.header.extensions.log_id();
                    let topic = crate::topic::resolve_application_topic(
                        &self.op_store.store,
                        &author,
                        &log_id,
                    )
                    .await?
                    .ok_or_else(|| {
                        anyhow!(
                            "failed to resolve topic for operation. this is a bug. author: {:?}, log: {:?}",
                            author.aliased(),
                            log_id.aliased()
                        )
                    })?;

                    if let (Some(media), Some(blob_sync)) = (m.media(), &self.blob_sync) {
                        let hashes: Vec<_> = media.iter().map(|item| item.hash()).collect();
                        self.blob_decrypt_index.remove(&hashes);
                        let is_own = DeviceId::from(author) == self.device_id();
                        if let Err(err) = self
                            .mailboxes
                            .blob_tracker()
                            .remove_all_mailboxes(&hashes)
                            .await
                        {
                            tracing::warn!(?err, "failed to clear unfetched blob rows on delete");
                        }
                        blob_sync
                            .delete_blobs(topic, author.into(), operation.hash, hashes, is_own)
                            .await;
                    }
                }
                _ => {
                    return Ok(());
                }
            }
        }
        Ok(())
    }

    async fn process_app(
        &self,
        operation: &ProcessedOperation<Payload>,
        source: &Source,
    ) -> anyhow::Result<()> {
        self.register_bootstrap(operation, source).await?;
        let topic = operation.topic();
        let header = operation.processed().header();

        let inbox_owner = match operation.message() {
            Payload::Inbox(_) => self.inbox_owner(topic).await?,
            _ => None,
        };

        // If an operation is invalidated by the projection layer, we don't process it,
        // but still allow it to be acknowledged as processed.
        match self
            .projection
            .reduce(
                self.agent_id(),
                self.device_id(),
                inbox_owner,
                operation,
                BadUseOfNode::from(self.clone()),
            )
            .await
        {
            // Continue processing.
            Ok(events) => {
                for event in events {
                    self.notify_system_event(event).await?;
                }
            }

            // Don't process but allow the log to proceed.
            Err(ProjectionError::InvalidOp(msg)) => {
                tracing::info!(msg, "invalid operation");
                return Ok(());
            }

            // Bubble up the error: the log won't proceed.
            Err(ProjectionError::Any(err)) => {
                return Err(err);
            }
        }

        // If a receiver is processing this, the tombstone may have been processed prior,
        // so we want to drop the payload now and not process it.
        if self.enforce_tombstone(topic, operation.id()).await? {
            // The payload is tombstoned, so we must not process it. Return early.
            self.notify_header(topic, header).await?;
            return Ok(());
        }

        let hash = operation.id();
        let author = DeviceId::from(operation.author());
        let payload = operation.message();

        match &payload {
            Payload::Chat(ChatPayload::JoinGroup { chat_id, .. }) => {
                self.join_chat_group(*chat_id)
                    .await
                    .context("failed to join group from invitation")?;
                // Push my profile into the new space.
                // Spawned: publishing needs this processor.
                if !source.is_local() {
                    let node = self.clone();
                    let chat_id = *chat_id;
                    tokio::spawn(async move {
                        if let Err(err) = node.publish_my_profile_to(chat_id).await {
                            tracing::warn!(?err, "failed to push my profile to a joined group");
                        }
                    });
                }
            }

            Payload::Inbox(invitation) => {
                let topic_id = TopicId::from(topic);
                let all_advertised_topics = self.local_store.get_advertised_inbox_topics().await?;
                let is_advertised_topic =
                    all_advertised_topics.iter().any(|it| *it.topic == topic_id);
                let is_reply = self
                    .local_store
                    .get_reply_inbox_topics()
                    .await?
                    .iter()
                    .any(|it| *it.topic == topic_id);
                if !is_advertised_topic && !is_reply {
                    // not for me (e.g. another scanner's request on a shared
                    // advertised inbox we only synced as an intermediary): ignore.
                    return Ok(());
                }
                match invitation {
                    InboxPayload::ContactRequest {
                        agent_id,
                        direct_chat_id,
                        ..
                    } => {
                        // A request arrived on our advertised inbox. Disclose
                        // nothing about us and register no bootstrap node until
                        // the user explicitly accepts (see `accept_contact`), so
                        // that an unsolicited request — e.g. anyone scanning a
                        // shared QR — is not handed our profile. All it gets is
                        // that we follow the space it names, without waking the
                        // device for it, until the request is rejected or its
                        // sender blocked. The request is signed by the
                        // scanner's device key (author), so we map that device to
                        // the requester's agent_id directly rather than trusting
                        // the embedded QR code's agent_id.
                        //
                        // TODO: maybe find a way to not rely on `source`?
                        if is_advertised_topic && !source.is_local() {
                            // If we also sent this peer a contact request,
                            // theirs is an implicit acceptance: no manual tap,
                            // and no reply either way. A read-only node (the
                            // push extension) leaves completing it to the app.
                            let crossed = !self.config.read_only
                                && self.has_outgoing_pending_request(author).await?;

                            // Join the requester's space now, so we can read
                            // what they write before we accept. Only if the
                            // projection recorded it as our chat with them: a
                            // request could name any space, even one of ours.
                            // A read-only node (the push extension) leaves
                            // following it to the app.
                            let direct_chat_id = *direct_chat_id;
                            if !self.config.read_only
                                && self.projection.direct_chat_peer(direct_chat_id).await?
                                    == Some(author)
                            {
                                let node = self.clone();
                                tokio::spawn(async move {
                                    if let Err(err) =
                                        node.register_and_join_space(direct_chat_id).await
                                    {
                                        tracing::warn!(
                                            ?err,
                                            "failed to join the requester's direct chat space"
                                        );
                                    }
                                });
                            }

                            // Spawned so we don't await publishing (which
                            // needs this same processor) and deadlock.
                            if crossed {
                                let node = self.clone();
                                let agent_id = *agent_id;
                                tokio::spawn(async move {
                                    if let Err(err) =
                                        node.complete_crossed_request(author, agent_id).await
                                    {
                                        tracing::warn!(
                                            ?err,
                                            "failed to complete crossed contact requests"
                                        );
                                    }
                                });
                            }
                        }
                    }
                    InboxPayload::ContactRequestAccept { agent_id, .. } => {
                        // The op must arrive on our private reply topic and be
                        // signed by the device whose QR we scanned. Verifying
                        // Verifying author == expected_ack_author prevents a
                        // third party from injecting a spoofed ack with
                        // an attacker-chosen agent_id/profile.
                        //
                        // TODO: maybe find a way to not rely on `source`?
                        if is_reply && !source.is_local() {
                            let expected = self
                                .local_store
                                .get_reply_inbox_expected_ack_author(topic_id)
                                .await?;
                            if expected.as_ref() != Some(&author) {
                                tracing::warn!(
                                    ?author,
                                    ?expected,
                                    "ContactRequestAccept author does not match expected; ignoring"
                                );
                                return Ok(());
                            }

                            // Crossed requests made us contacts already: an
                            // honest peer does not also answer ours.
                            if self.is_accepted_contact(author).await? {
                                return Ok(());
                            }

                            self.establish_contact(author).await?;

                            // A read-only node (the push extension) establishes
                            // the contact's topics so it can notify on their
                            // messages, but leaves recording the contact to the
                            // app, which re-processes this accept when it
                            // imports the extension's records.
                            if !self.config.read_only {
                                let node = self.clone();
                                let agent_id = *agent_id;
                                tokio::spawn(async move {
                                    // The accept op can be processed more than once
                                    // (it is replayed when its ack fails), and each
                                    // processing lands here; record the contact only
                                    // the first time.
                                    match node.accepted_contact_agent_ids().await {
                                        Ok(ids) if ids.contains(&agent_id) => return,
                                        Ok(_) => {}
                                        Err(err) => {
                                            tracing::warn!(
                                                ?err,
                                                "failed to check recorded contacts"
                                            );
                                            return;
                                        }
                                    }
                                    if let Err(err) =
                                        node.complete_accepted_request(author, agent_id).await
                                    {
                                        tracing::error!(
                                            ?err,
                                            "failed to complete our accepted contact request"
                                        );
                                    }
                                });
                            }
                        }
                    }
                }
            }

            Payload::Chat(ChatPayload::Message(m)) => {
                // Mirror the author-side reply validation, but only for
                // observability: an invalid reply annotation is ignored at
                // render time (the frontend applies the same rules), while the
                // message carrying it is still processed and shown.
                if let Some(target) = m.reply() {
                    let chat_id = ChatId::from_topic_id(topic);
                    let valid_ops = self.valid_chat_ops(chat_id).await?;
                    let candidate = crate::chat::ReplyCandidate {
                        target,
                        self_hash: Some(hash),
                    };
                    if let Err(err) = candidate.validate(&valid_ops) {
                        warn!(?err, op = ?hash.aliased(), "message carries an invalid reply annotation");
                    }
                }

                if let (Some(media), Some(blob_sync)) = (m.media(), &self.blob_sync) {
                    self.blob_decrypt_index.insert_blobs(topic.into(), media);
                    for item in media.iter() {
                        blob_sync
                            .add_to_fetch_pool(topic.into(), author, hash, item.hash())
                            .await?;
                    }
                }
            }

            Payload::Chat(ChatPayload::EditMessage { edit_hash, .. }) => {
                // Mirror the author-side validation: an edit that breaks the
                // linear-chain / authorship / window rules is ignored (not
                // forwarded to the frontend) with a warning.
                let chat_id = ChatId::from_topic_id(topic);
                let valid_ops = self.valid_chat_ops(chat_id).await?;
                let edit_ts: u64 = operation.processed().header().extensions.timestamp().into();
                let candidate = EditCandidate {
                    target: *edit_hash,
                    editor: author,
                    timestamp: edit_ts,
                    self_hash: Some(hash),
                };
                if let Err(err) = candidate.validate(&valid_ops) {
                    warn!(?err, op = ?hash.aliased(), "ignoring invalid edit message");
                    return Ok(());
                }
            }

            Payload::Chat(ChatPayload::DeleteMessage { hashes }) => {
                // Enforce the tombstones the projection just recorded, dropping
                // the targets' payloads. The delete op's *own* payload is only a
                // list of hashes (nothing sensitive), so we fall through to
                // `notify_payload` below: the frontend needs it to learn which
                // messages were deleted and render their placeholders.
                for hash in hashes {
                    self.enforce_tombstone(topic, *hash).await?;
                }
            }

            Payload::DeviceGroup(DeviceGroupPayload::DeleteForMe(delete)) => {
                // Drop the payloads referenced by the delete.
                //
                // Unlike `DeleteForEveryone`, a `DeleteForMe` op is never shared with the
                // other chat participants, so their copies are untouched. We fall
                // through to `notify_payload` below so the frontend re-reads the
                // tombstone set and drops the message from its chat view.
                let chat_topic: TopicId = delete.chat_id.into();
                let valid_ops = self.valid_chat_ops(delete.chat_id).await?;
                for hash in forward_edit_closure(&valid_ops, delete.message_hash) {
                    self.enforce_tombstone(chat_topic, hash).await?;
                }
            }

            Payload::Chat(ChatPayload::MessageAck { .. }) => {
                // Already folded into the projection by `reduce`; deliberately
                // never marks the topic dirty below, or two online peers would
                // ack each other's acks forever.
            }

            _ => {
                // Nothing to do.
            }
        }

        if let Payload::Chat(chat_payload) = &payload {
            if !matches!(chat_payload, ChatPayload::MessageAck { .. }) && author != self.device_id()
            {
                self.mark_ack_topic_dirty(ChatId::from_topic_id(topic));
                if self.projection.direct_chat_spaces(author).await?.len() > 1 {
                    let node = self.clone();
                    tokio::spawn(async move {
                        if let Err(err) = node.drop_moved_direct_chat_space(author).await {
                            warn!(?err, "failed to drop a moved direct chat space");
                        }
                    });
                }
            }
        }

        tracing::debug!(hash = ?hash.aliased(), "processed operation");

        // For all message types except groups control messages notify that a new payload has been
        // received.

        // @TODO: the application layer is informed of all events here even if something in the
        // processing resulted in an error. It might be required that the frontend is also
        // informed of any errors or these events are not even forwarded.

        self.notify_payload(topic, &operation.processed().header(), &payload)
            .await?;

        Ok(())
    }

    async fn register_bootstrap(
        &self,
        operation: &ProcessedOperation<Payload>,
        source: &Source,
    ) -> anyhow::Result<()> {
        // Dash Chat relies on mailbox servers for discovering bootstrap
        // nodes over the internet. Any operations we're sent from an
        // external stream is assumed to come from a mailbox, and we
        // attempt to register all authors we find as bootstrap nodes.
        //
        // @TODO: This is a temp solution and not a recommended way to
        // discover bootstraps because 1) not all authors will be online
        // and contactable for bootstrapping, 2) it locks all peers into
        // using the same relay. A better approach would be to have
        // mailbox servers notify connected nodes of each-others presence
        // via a dedicated channel.
        if let Source::ExternalStream { .. } = source {
            let node_id: NodeId = operation.author().into();
            self.register_bootstrap_node(node_id).await?;
        }

        Ok(())
    }

    /// Give p2panda a node's address on the shared relay, unless it already
    /// knows the node, so discovery can reach it. Deduplicated so repeated
    /// calls are cheap. Used
    /// both for mailbox-stream authors and for a freshly-scanned contact, the
    /// latter letting two nodes connect directly over the internet (relay +
    /// pkarr) without depending on a mutually-reachable mailbox.
    pub(crate) async fn register_bootstrap_node(&self, node_id: NodeId) -> anyhow::Result<()> {
        // In no-p2p mode we never register peers as bootstrap nodes, so direct
        // iroh connections between Dash Chat clients are never established. All
        // communication flows through mailbox servers.
        if !self.config.enable_p2p {
            return Ok(());
        }

        if !self
            .registered_bootstraps
            .lock()
            .await
            .insert((node_id, RELAY_URL.clone()))
        {
            return Ok(());
        }

        debug!(node_id = %node_id, "add bootstrap node");
        self.streams
            .register_bootstrap(node_id, RELAY_URL.clone())
            .await?;
        Ok(())
    }

    /// Notify the frontend of an operation without its payload. Used when the
    /// op's body has been tombstoned: the frontend must learn the op exists (so
    /// it refetches and renders the body-less op) but must never receive the
    /// deleted content.
    pub async fn notify_header(&self, topic: TopicId, header: &Header) -> anyhow::Result<()> {
        self.notify(
            OpNotification {
                topic,
                header: header.clone(),
                payload: None,
            }
            .into(),
        );
        Ok(())
    }

    pub async fn notify_payload(
        &self,
        topic: TopicId,
        header: &Header,
        payload: &Payload,
    ) -> anyhow::Result<()> {
        self.notify(
            OpNotification {
                topic,
                header: header.clone(),
                payload: Some(payload.clone()),
            }
            .into(),
        );
        Ok(())
    }

    /// Tell the frontend a group chat's membership changed. Membership changes
    /// of other spaces (direct chats, the device group) are of no interest to it.
    async fn notify_group_members_changed(&self, topic: TopicId) -> anyhow::Result<()> {
        let chat_id = ChatId::from_topic_id(topic);
        if self.projection.is_group_chat(chat_id).await? {
            self.notify_system_event(SystemNotification::GroupMembersChanged { chat_id })
                .await?;
        }
        Ok(())
    }

    async fn notify_system_event(&self, event: SystemNotification) -> anyhow::Result<()> {
        self.notify(event.into());
        Ok(())
    }

    /// Hand a notification to the frontend without waiting on it: the processor drives
    /// p2panda's bounded pipeline, so a webview that stops draining must not stall sync.
    fn notify(&self, notification: Notification) {
        let Some(notification_tx) = &self.notification_tx else {
            return;
        };
        match notification_tx.try_send(notification) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => {
                warn!("notification channel full (frontend not draining), dropping notification")
            }
            Err(TrySendError::Closed(_)) => warn!("notification channel closed"),
        }
    }
}
