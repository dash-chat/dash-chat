use anyhow::anyhow;
use futures::StreamExt;
use p2panda::NodeId;
use p2panda::operation::Header;
use p2panda::streams::{ProcessedOperation, Source, StreamEvent};
use serde::{Deserialize, Serialize};
use tokio_stream::wrappers::ReceiverStream;
use tracing::{debug, warn};

use crate::node::actor::{ProcessorError, ProcessorEvent};
use crate::topic::AutoRegisteredTopic;

use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Notification {
    pub topic: Topic,
    pub header: Header,
    pub payload: Option<Payload>,
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

                                let result = node.process_groups(operation, &source).await.map_err(|err|ProcessorError::App(err.to_string()));
                                if let Err(err) = result.as_ref() {
                                    tracing::error!(?err, "process groups operation error");
                                };

                                // Signal that the operation has been fully processed. This will
                                // allow the ProcessFuture to complete. We return a result here so
                                // that any errors can be reacted to by the waiter.
                                if let Some(processed_tx) = processed_tx {
                                    if let Err(err) = processed_tx.send(result) {
                                        tracing::error!(?err, "processed_tx send error")
                                    }
                                }

                                // @TODO: this is required for tests, but nowhere else, it can be placed behind the
                                // testing flag.
                                node.op_store.mark_op_processed(topic, &id);

                            },
                            ProcessorEvent::App { operation, source, processed_tx } => {
                                let topic = operation.topic();
                                let id = operation.id();
                                tracing::info!(op = ?id.aliased(), topic = ?topic.aliased(), "application operation processing");

                                // Process the operation.
                                let result = node.process_app(operation, &source).await.map_err(|err|ProcessorError::App(err.to_string()));
                                if let Err(err) = result.as_ref() {
                                    tracing::error!(?err, "process operation error");
                                }

                                // Signal that the operation has been fully processed. This will
                                // allow the ProcessFuture to complete. We return a result here so
                                // that any errors can be reacted to by the waiter.
                                if let Some(processed_tx) = processed_tx {
                                    if let Err(err) = processed_tx.send(result) {
                                        tracing::error!(?err, "processed_tx send error")
                                    }


                                }

                                // @TODO: this is required for tests, but nowhere else, it can be placed behind the
                                // testing flag.
                                node.op_store.mark_op_processed(topic, &id);

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

    /// Enforce the topic's tombstone set on a received operation: if its hash
    /// has been tombstoned, drop its stored payload so it is never persisted or
    /// synced onward. The operation arrives here already written to the op
    /// store (by peers or by mailbox sync), so this deletes the body after the
    /// fact.
    async fn enforce_tombstone(
        &self,
        operation: &ProcessedOperation<Payload>,
    ) -> anyhow::Result<bool> {
        let topic = operation.topic();
        let hash = operation.id();
        if self.local_store.is_tombstoned(topic, hash).await? {
            self.unprocess_app(&operation.processed().operation).await?;
            self.op_store.delete_body(&hash).await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Filter out operations whose payloads are not able to be deleted.
    pub(crate) fn is_tombstoneable(&self, payload: &Payload) -> bool {
        matches!(payload, Payload::Chat(ChatPayload::Message(_)))
    }

    /// Note that this is a function that processes operations which could have deleted payloads.
    /// This function currently doesn't do anything with payloads, so we don't check for tombstones here.
    /// If this function is modified to process payloads, then we should call
    /// [`Self::enforce_tombstone`] before processing.
    async fn process_groups(
        &self,
        operation: ProcessedOperation<Payload>,
        source: &Source,
    ) -> anyhow::Result<()> {
        self.register_bootstrap(&operation, source).await?;

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
            .local_store
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
        let dashchat_topic =
            crate::Topic::<crate::topic::kind::Untyped>::new(*operation.topic().as_bytes());
        self.notify_payload(
            dashchat_topic,
            &operation.processed().header(),
            operation.message(),
        )
        .await?;

        Ok(())
    }

    /// Undo the effects of processing an app-layer operation.
    /// This is done when an operation payload is deleted.
    /// Only tombstoneable operations can be unprocessed.
    pub(crate) async fn unprocess_app(&self, operation: &Operation) -> anyhow::Result<()> {
        let Some(payload) = Payload::try_from_body_opt(operation.body.as_ref())? else {
            return Ok(());
        };
        if self.is_tombstoneable(&payload) {
            match payload {
                Payload::Chat(ChatPayload::Message(m)) => {
                    use p2panda_store::topics::TopicStore;
                    let author = operation.header().verifying_key;
                    let log_id = operation.header.extensions.log_id;
                    let topic = self.op_store.store.resolve_topic(&author, &log_id).await?;
                    let Some(topic) = topic else {
                        tracing::error!("failed to resolve topic for operation: {operation:?}");
                        return Ok(());
                    };
                    if let Some(media) = m.media() {
                        let hashes: Vec<_> = media.iter().map(|item| item.hash).collect();
                        self.blob_sync.delete_blobs(topic, hashes).await;
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
        operation: ProcessedOperation<Payload>,
        source: &Source,
    ) -> anyhow::Result<()> {
        self.register_bootstrap(&operation, source).await?;
        let topic = operation.topic();
        let dashchat_topic = crate::Topic::new(*topic.as_bytes());
        let header = operation.processed().header();

        // NOTE: realistically the tombstone will only be enforced here
        // upon playback of operations. The first time through, it will
        // have been processed and potentially have modified local state.
        // Upon tombstoning (or enforcement), it will be "unprocessed"
        // via [`Self::unprocess_app`] to undo those state changes,
        // as if it were never processed at all. On playback, the operation
        // simply doesn't get processed.
        if self.enforce_tombstone(&operation).await? {
            // The payload is tombstoned, so there's nothing to process.
            self.notify_header(dashchat_topic, header).await?;
            return Ok(());
        }

        let hash = operation.id();
        let device_id = DeviceId::from(operation.author());
        let payload = operation.message();

        match &payload {
            Payload::GroupControl(_) => {
                // Nothing to do.
            }
            Payload::Chat(ChatPayload::IntroduceAgents { agents }) => {
                tracing::info!(
                    me = ?device_id.aliased(),
                    count = agents.len(),
                    "received IntroduceAgents message"
                );
                for (device_id, agent_id) in agents {
                    if let Err(err) = self
                        .local_store
                        .save_agent_mapping(*device_id, *agent_id)
                        .await
                    {
                        tracing::warn!(
                            ?err,
                            device_id = ?device_id.aliased(),
                            agent_id = ?agent_id.aliased(),
                            "failed to save agent mapping from IntroduceAgents"
                        );
                    }
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
                if let Err(err) = self.join_group(*chat_id).await {
                    // TODO: no retry path — device ends up with no topic registered for this group.
                    tracing::error!(?err, "failed to join group from invitation");
                }
            }

            Payload::Inbox(invitation) => {
                let active_topics = self.local_store.get_active_inbox_topics().await?;
                if !active_topics
                    .iter()
                    .any(|it| *it.topic == TopicId::from(topic))
                {
                    // not for me, ignore
                    return Ok(());
                }
                match invitation {
                    InboxPayload::ContactRequest { .. } => {
                        // Nothing to do.
                    }
                }
            }

            Payload::Chat(ChatPayload::Message(m)) => {
                if let Some(media) = m.media() {
                    for item in media.iter() {
                        // TODO: revisit during ACID review (replay)
                        self.blob_sync.fetch_pool.add(topic.into(), item.hash).await;
                    }
                }
            }

            Payload::Chat(ChatPayload::Reaction(_) | ChatPayload::GroupInfo(_)) => {
                // Nothing to do.
            }

            Payload::Announcements(AnnouncementsPayload::SetProfile(profile)) => {
                // HACK: The announcements topic id IS the agent_id bytes, so we can reconstruct it here.
                let agent_id =
                    AgentId::from(crate::ActorId::from_bytes(topic.as_bytes()).map_err(|e| {
                        anyhow::anyhow!("invalid agent_id bytes in announcements topic: {e}")
                    })?);

                tracing::info!(me = ?self.agent_id().aliased(), agent_id = ?agent_id.aliased(), ?profile, "save_profile");

                if let Err(err) = self
                    .local_store
                    .save_profile(agent_id, profile.clone())
                    .await
                {
                    tracing::warn!(?err, "failed to save profile from SetProfile");
                }
            }

            Payload::Announcements(AnnouncementsPayload::SetCapabilities { capabilities }) => {
                // Save the device_id -> agent_id mapping so group members can look each other up.

                // HACK: The announcements topic id IS the agent_id bytes, so we can reconstruct it here.
                let agent_id =
                    AgentId::from(crate::ActorId::from_bytes(topic.as_bytes()).map_err(|e| {
                        anyhow::anyhow!("invalid agent_id bytes in announcements topic: {e}")
                    })?);
                if let Err(err) = self
                    .local_store
                    .save_agent_mapping(device_id, agent_id)
                    .await
                {
                    tracing::warn!(?err, "failed to save agent mapping from SetCapabilities");
                }

                if let Err(err) = self
                    .local_store
                    .save_capabilities(device_id, capabilities.clone())
                    .await
                {
                    tracing::warn!(?err, "failed to save capabilities from SetCapabilities");
                }
            }

            Payload::DeviceGroup(_) => {
                // Nothing to do.
            }
        }

        tracing::debug!(hash = ?hash.aliased(), "processed operation");

        // For all message types except groups control messages notify that a new payload has been
        // received.

        // @TODO: the application layer is informed of all events here even if something in the
        // processing resulted in an error. It might be required that the frontend is also
        // informed of any errors or these events are not even forwarded.

        // We convert the p2panda::Topic into a dashchat Topic here in its untyped form.
        self.notify_payload(dashchat_topic, &operation.processed().header(), &payload)
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

            // Only register the bootstrap if we didn't already do so.
            if self
                .registered_bootstraps
                .lock()
                .await
                .insert((node_id, RELAY_URL.clone()))
            {
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
            };
        }

        Ok(())
    }

    pub async fn notify_header(&self, topic: Topic, header: &Header) -> anyhow::Result<()> {
        if let Some(notification_tx) = self.notification_tx.clone() {
            notification_tx
                .send(Notification {
                    topic: topic.clone(),
                    header: header.clone(),
                    payload: None,
                })
                .await
                .unwrap_or_else(|_| tracing::warn!("notification channel closed"));
        }
        Ok(())
    }

    pub async fn notify_payload(
        &self,
        topic: Topic,
        header: &Header,
        payload: &Payload,
    ) -> anyhow::Result<()> {
        if let Some((notification_tx, payload)) = self.notification_tx.clone().zip(Some(payload)) {
            notification_tx
                .send(Notification {
                    topic: topic.clone(),
                    header: header.clone(),
                    payload: Some(payload.clone()),
                })
                .await
                .unwrap_or_else(|_| tracing::warn!("notification channel closed"));
        }
        Ok(())
    }
}
