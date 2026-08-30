use anyhow::anyhow;
use derive_more::derive::From;
use futures::StreamExt;
use p2panda::NodeId;
use p2panda::operation::Header;
use p2panda::streams::{ProcessedOperation, Source, StreamEvent};
use serde::{Deserialize, Serialize};
use tokio_stream::wrappers::ReceiverStream;
use tracing::{debug, warn};

use crate::AckedOp;
use crate::forward_edit_closure;
use crate::node::actor::{ProcessorError, ProcessorEvent};
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
    pub header: Header,
    pub payload: Option<Payload>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum SystemNotification {
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
    /// When the node restarts, the topic will be reinitialized.
    ///
    /// Note that some topics are excluded from automatic registration, such as inbox topics.
    /// They have to be registered separately with extra context.
    pub(crate) async fn register_topic<K: AutoRegisteredTopic>(
        &self,
        topic: Topic<K>,
    ) -> anyhow::Result<()> {
        self.local_store.register_topic_as_subscribed(topic).await?;
        self.initialize_topic(*topic).await?;

        Ok(())
    }

    /// Internal function to start the necessary tasks for processing network activity
    /// for a given topic.
    ///
    /// This must be called:
    /// - when creating a new group chat
    /// - when initializing the node, for each existing group chat
    pub(crate) async fn initialize_topic(&self, topic: TopicId) -> anyhow::Result<()> {
        topic.alias_numbered();

        if self.subscribe_to_topic(topic).await? {
            self.import_mailbox_stream(topic).await?;
        };
        Ok(())
    }

    /// Subscribe to a topic.
    async fn subscribe_to_topic(&self, topic: TopicId) -> anyhow::Result<bool> {
        debug!(topic = ?topic.aliased(), "subscribe to topic");

        let (reply_tx, reply_rx) = oneshot::channel();
        if self
            .actor_tx
            .send(Command::Subscribe {
                topic: topic.into(),
                reply_tx,
            })
            .await
            .is_err()
        {
            return Err(anyhow!("Error sending on actor channel"));
        };

        let subscribed = reply_rx.await??;

        if let Some(tx) = &self.topic_subscribed_tx {
            let _ = tx.send(topic).await;
        }

        Ok(subscribed)
    }

    /// Import external operation stream from a mailbox.
    async fn import_mailbox_stream(&self, topic: TopicId) -> anyhow::Result<()> {
        debug!(topic = ?topic.aliased(), "import mailbox stream");

        let Some(mailbox_rx) = self.mailboxes.subscribe(topic.into()).await? else {
            tracing::warn!("topic already initialized, skipping");
            return Ok(());
        };

        let stream = Box::pin(ReceiverStream::new(mailbox_rx).map(Operation::from));
        let (reply_tx, reply_rx) = oneshot::channel();
        if self
            .actor_tx
            .send(Command::Import {
                topic: topic.into(),
                stream,
                reply_tx,
            })
            .await
            .is_err()
        {
            return Err(anyhow!("Error sending on actor channel"));
        };

        let _ = reply_rx.await?;

        Ok(())
    }

    /// Spawn a task for application layer processing of received operations.
    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all, fields(me=?self.device_id().aliased())))]
    pub(super) fn spawn_application_processor_task(
        &self,
        mut events_rx: mpsc::Receiver<ProcessorEvent>,
        mut cancel_rx: mpsc::Receiver<()>,
    ) -> tokio::task::JoinHandle<()> {
        let node = self.clone();

        let handle = tokio::spawn(async move {
            let node = node.clone();

            loop {
                tokio::select! {
                    Some(processor_event) = events_rx.recv() => {
                        match processor_event {
                            ProcessorEvent::System(event) => {
                                match event {
                                    StreamEvent::ProcessingFailed { error, .. } => warn!("error processing operation: {error:?}"),
                                    StreamEvent::DecodeFailed { error, .. } => warn!("error decoding operation: {error:?}"),
                                    StreamEvent::ReplayFailed { error, .. } => warn!("error replaying stream: {error:?}"),
                                    StreamEvent::AckFailed { error, .. } => warn!("error acking operation: {error:?}"),
                                    // @TODO: the operation variant should never be included here in the
                                    // system event as it will either be a groups or application event.
                                    StreamEvent::Processed {..} => unreachable!(),
                                    // @TODO: There are more interesting events which could be logged here.
                                    _ => ()
                                }
                            },
                            ProcessorEvent::Groups { operation, source, processed_tx, error } => {
                                let topic = operation.topic();
                                let id = operation.id();
                                tracing::info!(op = ?id.aliased(), topic = ?topic.aliased(), "groups operation processing");

                                if let Some(err) = error {
                                    // @TODO: should consider if this is the desired behavior.
                                    //
                                    // An error occurred when processing this groups operation so we skip
                                    // processing here on the application layer.
                                    if let Some(processed_tx) = processed_tx {
                                        if let Err(err) = processed_tx.send(Err(err)) {
                                            tracing::error!(?err, "processed_tx send error")
                                        }
                                    }

                                    continue;
                                };

                                let result = node.process_groups(&operation, &source).await.map_err(|err|ProcessorError::App(err.to_string()));

                                // Signal that the operation has been fully processed. This will
                                // allow the ProcessFuture to complete. We return a result here so
                                // that any errors can be reacted to by the waiter.
                                if let Some(processed_tx) = processed_tx {
                                    if let Err(err) = processed_tx.send(result.clone()) {
                                        tracing::error!(?err, "processed_tx send error")
                                    }
                                }

                                // Don't continue to acknowledgement if there was an error processing,
                                // so that the operation will be replayed another time.
                                if let Err(err) = result {
                                    tracing::error!(?err, "process groups operation error");
                                    continue;
                                };

                                if let Err(err) = node.ack_operation(&operation).await {
                                    tracing::error!(?err, "failed to acknowledge operation");
                                }
                            },
                            ProcessorEvent::App { operation, source, processed_tx } => {
                                let topic = operation.topic();
                                let id = operation.id();
                                tracing::info!(op = ?id.aliased(), topic = ?topic.aliased(), "application operation processing");


                                // Process the operation.
                                let result = node.process_app(&operation, &source).await.map_err(|err|ProcessorError::App(err.to_string()));

                                // Signal that the operation has been fully processed. This will
                                // allow the ProcessFuture to complete. We return a result here so
                                // that any errors can be reacted to by the waiter.
                                if let Some(processed_tx) = processed_tx {
                                    if let Err(err) = processed_tx.send(result.clone()) {
                                        tracing::error!(?err, "processed_tx send error")
                                    }
                                }

                                // Don't continue to acknowledgement if there was an error processing,
                                // so that the operation will be replayed another time.
                                if let Err(err) = result {
                                    tracing::error!(?err, "process operation error");
                                    continue;
                                }


                                if let Err(err) = node.ack_operation(&operation).await {
                                    tracing::error!(?err, "failed to acknowledge operation");
                                }
                            },
                        }

                    }
                    Some(()) = cancel_rx.recv() => {
                        tracing::info!("stream processing loop cancelled");
                        break;
                    }

                    else => {
                        // Both stream_rx is closed and streams is exhausted
                        break;
                    }
                }
            }

            tracing::info!("stream processing loop finished");
        });

        handle
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

        Ok(())
    }

    /// Enforce the topic's tombstone set on a received operation: if its hash
    /// has been tombstoned, drop its stored payload so it is never persisted or
    /// synced onward. The operation arrives here already written to the op
    /// store (by peers or by mailbox sync), so this deletes the body after the
    /// fact.
    //
    // TODO: when device groups exist, test that tombstones are enforced across devices.
    async fn enforce_tombstone(
        &self,
        topic: TopicId,
        tombstoned_op: &Operation,
    ) -> anyhow::Result<bool> {
        let hash = tombstoned_op.hash;
        if self.projection.is_tombstoned(topic, hash).await? {
            self.unprocess_app(tombstoned_op).await?;
            self.op_store.delete_body(&hash).await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Filter out operations whose payloads are not able to be deleted.
    pub(crate) fn is_tombstoneable(payload: &Payload) -> bool {
        matches!(
            payload,
            Payload::Chat(ChatPayload::Message(_) | ChatPayload::EditMessage { .. })
        )
    }

    /// Note that this is a function that processes operations which could have deleted payloads.
    /// This function currently doesn't do anything with payloads, so we don't check for tombstones here.
    /// If this function is modified to process payloads, then we should call
    /// [`Self::enforce_tombstone`] before processing.
    async fn process_groups(
        &self,
        operation: &ProcessedOperation<Payload>,
        source: &Source,
    ) -> anyhow::Result<()> {
        self.register_bootstrap(operation, source).await?;

        // If an operation is invalidated by the projection layer, we don't process it,
        // but still allow it to be acknowledged as processed.
        match self
            .projection
            .reduce(self.agent_id(), operation, BadUseOfNode::from(self.clone()))
            .await
        {
            // Continue processing.
            Ok(_) => (),

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

        // Subscribe to announcements topics for any group members whose agent_id we know.
        let topic = operation.topic();
        let topic = ChatId::from_topic_id(topic)?;

        // Calculate the current group membership based on local store state rather than
        // looking into the group actions themselves.
        //
        // @TODO: currently we're receiving group control operations here out-of-order but
        // soon the node will be doing ordering for us and operations will be released
        // only once their dependencies are met, which is the desired behavior. Once that
        // change occurs we can also receive current member state, or better just a diff,
        // in the node event and use that to understand who has been added/removed. For
        // now we get _all_ members and (possibly redundantly) attempt to subscribe to
        // them all. Even with the messages arriving out-of-order this will result in us
        // subscribing to all the correct topics.
        let member_device_ids: Vec<DeviceId> = self
            .group_store
            .members(topic)
            .await?
            .iter()
            .map(|(id, _)| *id)
            .collect();

        // @TODO: when removals are supported we should also unsubscribe from topics of
        // removed members.

        // Retrieve the agent_ids from the member_device_ids.
        //
        // @TODO: this requires a reliable way to know the agent id from the device id
        // even if they're not a contact.
        let known = self
            .projection
            .lookup_contacts(member_device_ids.iter())
            .await?;

        for agent_id in known.into_values() {
            let topic = Topic::announcements(agent_id);
            self.initialize_topic(*topic).await.map_err(|err| {
                anyhow::anyhow!("failed to subscribe to topic for group member: {err}")
            })?;
        }

        // Notify the frontend so it refetches the chat log and observes the new
        // group-control action (Create / Add / Remove / Promote / Demote). The
        // op's `auth.action` is what the UI reads to render membership changes.
        //
        // @TODO: once group control messages are properly ordered we could send a
        // membership diff here instead of relying on the frontend to refetch.
        self.notify_payload(
            operation.topic(),
            operation.processed().header(),
            operation.message(),
        )
        .await?;

        Ok(())
    }

    /// Undo the effects of processing an app-layer operation,
    /// reverting to a state as if the operation were never processed.
    /// This is done when an operation payload is deleted.
    /// Only tombstoneable operations can be unprocessed.
    pub(crate) async fn unprocess_app(&self, operation: &Operation) -> anyhow::Result<()> {
        let Some(payload) = Payload::try_from_body_opt(operation.body.as_ref())? else {
            return Ok(());
        };
        if Self::is_tombstoneable(&payload) {
            match payload {
                Payload::Chat(ChatPayload::Message(m)) => {
                    use p2panda_store::topics::TopicStore;
                    let author = operation.header().verifying_key;
                    let log_id = operation.header.extensions.log_id;
                    let topic = self
                        .op_store
                        .store
                        .resolve_topic(&author, &log_id)
                        .await?
                        .ok_or_else(|| {
                            anyhow!(format!("failed to resolve topic for operation. this is a bug. author: {:?}, log: {:?}", author.aliased(), log_id.aliased()))
                        })?;

                    if let (Some(media), Some(blob_sync)) = (m.media(), &self.blob_sync) {
                        let hashes: Vec<_> = media.iter().map(|item| item.hash()).collect();
                        let is_own = DeviceId::from(author) == self.device_id();
                        if let Err(err) = self
                            .local_store
                            .remove_unfetched_blobs_all_mailboxes(&hashes)
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

        // If an operation is invalidated by the projection layer, we don't process it,
        // but still allow it to be acknowledged as processed.
        match self
            .projection
            .reduce(self.agent_id(), operation, BadUseOfNode::from(self.clone()))
            .await
        {
            // Continue processing.
            Ok(event) => {
                if let Some(event) = event {
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
        if self
            .enforce_tombstone(topic, &operation.event.operation)
            .await?
        {
            // The payload is tombstoned, so we must not process it. Return early.
            self.notify_header(topic, header).await?;
            return Ok(());
        }

        let hash = operation.id();
        let author = DeviceId::from(operation.author());
        let payload = operation.message();

        match &payload {
            Payload::GroupControl(_) => {
                // Nothing to do.
            }
            Payload::Chat(ChatPayload::IntroduceAgents { agents }) => {
                for (_, agent_id) in agents {
                    if agent_id == &self.agent_id() {
                        continue;
                    }
                    if let Err(err) = self.register_topic(Topic::announcements(*agent_id)).await {
                        tracing::error!(
                            ?err,
                            agent_id = ?agent_id.aliased(),
                            "failed to register announcements topic for IntroduceAgents"
                        );
                    }
                }
            }

            Payload::Chat(ChatPayload::JoinGroup { chat_id }) => {
                self.join_group(*chat_id)
                    .await
                    .context("failed to join group from invitation")?;
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
                    InboxPayload::ContactRequest { agent_id, .. } => {
                        // A request arrived on our advertised inbox. Except for
                        // subscribing to the shared direct-chat topic (so the
                        // requester's messages are readable before accepting),
                        // perform no network side-effects (bootstrap
                        // registration) and disclose nothing about us until the
                        // user explicitly accepts (see `accept_contact`). This
                        // keeps an unsolicited request — e.g. anyone scanning a
                        // shared QR — from amplifying our resources or handing our
                        // profile to every scanner. The request is signed by the
                        // scanner's device key (author), so we map that device to
                        // the requester's agent_id directly rather than trusting
                        // the embedded QR code's agent_id.
                        if is_advertised_topic && !matches!(source, Source::LocalStore) {
                            self.register_topic(
                                self.direct_chat_topic(crate::FakeAgentId::from(author)),
                            )
                            .await?;
                            // Mutual add: if we also sent this peer a contact
                            // request, their incoming request is an implicit
                            // acceptance — complete the exchange automatically
                            // rather than waiting for a manual tap. Spawned so we
                            // don't await publishing (which needs this same
                            // processor) and deadlock.
                            if self.has_outgoing_pending_request(author).await? {
                                let node = self.clone();
                                let agent_id = *agent_id;
                                tokio::spawn(async move {
                                    if let Err(err) = node.accept_contact(agent_id).await {
                                        tracing::warn!(
                                            ?err,
                                            "failed to auto-accept mutual contact request"
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
                        if is_reply && !matches!(source, Source::LocalStore) {
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
                            self.establish_contact(author, *agent_id).await?;

                            let node = self.clone();
                            let agent_id = *agent_id;
                            tokio::spawn(async move {
                                let topic_id =
                                    node.direct_chat_topic(crate::FakeAgentId::from(author));
                                if let Err(err) = node.publish_add_contact(agent_id, topic_id).await
                                {
                                    tracing::warn!(?err, "failed to record accepted contact");
                                    return;
                                }
                                // Messages they sent before we recorded the
                                // acceptance were processed but dropped by the
                                // ack filter; now that they are a contact,
                                // cover them.
                                node.mark_ack_topic_dirty(topic_id);
                            });
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
                    let chat_id = ChatId::from_topic_id(topic)?;
                    let valid_ops = self.valid_chat_ops(chat_id).await?;
                    let candidate = crate::chat::ReplyCandidate {
                        target,
                        timestamp: operation.processed().header().timestamp.into(),
                        self_hash: Some(hash),
                    };
                    if let Err(err) = candidate.validate(&valid_ops) {
                        warn!(?err, op = ?hash.aliased(), "message carries an invalid reply annotation");
                    }
                }

                if let (Some(media), Some(blob_sync)) = (m.media(), &self.blob_sync) {
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
                let chat_id = ChatId::from_topic_id(topic)?;
                let valid_ops = self.valid_chat_ops(chat_id).await?;
                let edit_ts: u64 = operation.processed().header().timestamp.into();
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
                    if let Some(op) = self.op_store.get_operation(hash).await? {
                        self.enforce_tombstone(topic, &op).await?;
                    }
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
                    if let Some(op) = self.op_store.get_operation(&hash).await? {
                        self.enforce_tombstone(chat_topic, &op).await?;
                    }
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
                // Non-fatal: failing here must not fail (and so replay) the op.
                match ChatId::from_topic_id(topic) {
                    Ok(chat_id) => self.mark_ack_topic_dirty(chat_id),
                    Err(err) => warn!(?err, "chat payload on non-chat topic; not marking for ack"),
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

    /// Register a single node as a bootstrap (on the shared relay) so p2panda
    /// discovery can reach it. Deduplicated so repeated calls are cheap. Used
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
        let (reply_tx, reply_rx) = oneshot::channel();
        self.actor_tx
            .send(Command::RegisterBootstrap {
                node_id,
                relay_url: RELAY_URL.clone(),
                reply_tx,
            })
            .await
            .map_err(|err| anyhow::anyhow!("send to actor error: {err}"))?;

        reply_rx.await??;
        Ok(())
    }

    /// Notify the frontend of an operation without its payload. Used when the
    /// op's body has been tombstoned: the frontend must learn the op exists (so
    /// it refetches and renders the body-less op) but must never receive the
    /// deleted content.
    pub async fn notify_header(&self, topic: TopicId, header: &Header) -> anyhow::Result<()> {
        if let Some(notification_tx) = self.notification_tx.clone() {
            notification_tx
                .send(
                    OpNotification {
                        topic: topic.clone(),
                        header: header.clone(),
                        payload: None,
                    }
                    .into(),
                )
                .await
                .unwrap_or_else(|_| tracing::warn!("notification channel closed"));
        }
        Ok(())
    }

    pub async fn notify_payload(
        &self,
        topic: TopicId,
        header: &Header,
        payload: &Payload,
    ) -> anyhow::Result<()> {
        if let Some((notification_tx, payload)) = self.notification_tx.clone().zip(Some(payload)) {
            notification_tx
                .send(
                    OpNotification {
                        topic: topic.clone(),
                        header: header.clone(),
                        payload: Some(payload.clone()),
                    }
                    .into(),
                )
                .await
                .unwrap_or_else(|_| tracing::warn!("notification channel closed"));
        }
        Ok(())
    }

    async fn notify_system_event(&self, event: SystemNotification) -> anyhow::Result<()> {
        if let Some(notification_tx) = self.notification_tx.clone() {
            notification_tx
                .send(event.into())
                .await
                .unwrap_or_else(|_| tracing::warn!("notification channel closed"));
        }
        Ok(())
    }
}
