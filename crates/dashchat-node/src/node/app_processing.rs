use anyhow::anyhow;
use futures::StreamExt;
use p2panda::{operation::Header, streams::ProcessedOperation};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use tokio_stream::wrappers::ReceiverStream;
use tracing::debug;

use crate::topic::AutoRegisteredTopic;

use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Notification {
    pub header: Header,
    pub payload: Payload,
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
        if self.subscribe_to_topic(topic).await? {
            self.import_mailbox_stream(topic).await?;
        };
        Ok(())
    }

    /// Subscribe to a topic.
    async fn subscribe_to_topic(&self, topic: TopicId) -> anyhow::Result<bool> {
        debug!(topic = %topic, "subscribe to topic");

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

        // @TODO: I'm not sure what this notification channel is for.
        if let Some(tx) = &self.topic_subscribed_tx {
            let _ = tx.send(topic).await;
        }

        Ok(subscribed)
    }

    /// Import external operation stream from a mailbox.
    async fn import_mailbox_stream(&self, topic: TopicId) -> anyhow::Result<()> {
        debug!(topic = %topic, "import mailbox stream");

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
    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all, fields(me=?self.device_id().renamed())))]
    pub(super) fn spawn_application_processor_task(
        &self,
        mut operations_rx: broadcast::Receiver<ProcessedOperation<Payload>>,
        mut cancel_rx: mpsc::Receiver<()>,
    ) -> tokio::task::JoinHandle<()> {
        let node = self.clone();

        let handle = tokio::spawn(async move {
            let node = node.clone();

            loop {
                tokio::select! {
                    Ok(op) = operations_rx.recv() => {
                        tracing::info!(op = %op.id(), topic = %op.topic(), "processing stream item");
                        if let Err(err) = node.process_stream_item(op).await {
                            tracing::error!(?err, "process stream item error");
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

    async fn process_stream_item(
        &self,
        operation: ProcessedOperation<Payload>,
    ) -> anyhow::Result<()> {
        let hash = operation.id();
        let topic = operation.topic();
        let author = operation.author();
        let payload = operation.message();

        match payload {
            Payload::Chat(ChatPayload::JoinGroup { .. }) => {
                // Nothing to do.
            }

            Payload::Inbox(_invitation) => {
                // Nothing to do.
            }

            Payload::Chat(ChatPayload::Message(_) | ChatPayload::Reaction(_)) => {
                // Nothing to do.
            }

            Payload::Announcements(AnnouncementsPayload::SetProfile(profile)) => {
                // HACK: The announcements topic id IS the agent_id bytes, so we can reconstruct
                // it here.

                // The agent_id is the root identity for a person, it will be mapped to
                // device_id's in the future.
                let agent_id =
                    AgentId::from(crate::ActorId::from_bytes(topic.as_bytes()).map_err(|e| {
                        anyhow::anyhow!("invalid agent_id bytes in announcements topic: {e}")
                    })?);

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

                let device_id = DeviceId::from(author);
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
            Payload::GroupControl(_) => {
                // Subscribe to announcements topics for any group members whose agent_id we know.
                let topic = ChatId::from_topic(topic);

                // Calculate the current group membership based on local store state
                // rather than looking into the group actions themselves.

                // @TODO: currently we're processing operations here out-of-order but soon the
                // node will be doing ordering for us and operations will be "released" only once
                // their dependencies are met, which is the desired behavior.
                let member_device_ids: Vec<DeviceId> = self
                    .group_store
                    .members(topic)
                    .await?
                    .iter()
                    .map(|(id, _)| *id)
                    .collect();

                // @TODO: when removals are support we should also unsubscribe from topics of
                // removed members.

                // Retrieve the agent_ids from the member_device_ids.

                // @TODO: this requires a reliable way to know the agent id from the device id
                // even if they're not a contact.
                let known = self
                    .local_store
                    .lookup_contacts(member_device_ids.iter())
                    .await?;

                for agent_id in known.into_values() {
                    let topic = Topic::announcements(agent_id);
                    if let Err(err) = self.initialize_topic(*topic).await {
                        tracing::warn!(
                            ?err,
                            "failed to subscribe to announcements topic for group member"
                        );
                    }
                }
            }
        }

        tracing::debug!(hash = %hash, "processed operation");

        // For all message types except groups control messages notify that a new payload has been
        // received.

        // @TODO: once group control messages are properly ordered then we could also send these
        // to the frontend.
        if !matches!(payload, Payload::GroupControl(_)) {
            self.notify_payload(&operation.processed().header(), payload)
                .await?;
        }

        // @TODO: this is required for tests, but nowhere else, it can be placed behind the
        // testing flag.
        self.op_store.mark_op_processed(topic.into(), &hash);

        Ok(())
    }

    // @TODO: move to application processor.
    pub async fn notify_payload(&self, header: &Header, payload: &Payload) -> anyhow::Result<()> {
        if let Some((notification_tx, payload)) = self.notification_tx.clone().zip(Some(payload)) {
            notification_tx
                .send(Notification {
                    header: header.clone(),
                    payload: payload.clone(),
                })
                .await
                .unwrap_or_else(|_| tracing::warn!("notification channel closed"));
        }
        Ok(())
    }
}
