use std::pin::Pin;

use futures::Stream;
use futures::StreamExt;
use futures::stream::SelectAll;
use p2panda_stream::StreamLayerExt;
use p2panda_stream::ingest::Ingest;
use p2panda_stream::ingest::IngestArgs;
use p2panda_stream::ingest::IngestResult;
use serde::{Deserialize, Serialize};
use tokio_stream::wrappers::ReceiverStream;
use tracing::Instrument;

use crate::LogId;
use crate::{payload::InboxPayload, topic::AutoRegisteredTopic};

use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Notification {
    pub header: Header,
    pub payload: Payload,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Event {
    pub operation: Operation,
    pub args: IngestArgs<LogId, TopicId>,
}

impl std::borrow::Borrow<IngestArgs<LogId, TopicId>> for Event {
    fn borrow(&self) -> &IngestArgs<LogId, TopicId> {
        &self.args
    }
}

impl std::borrow::Borrow<Operation> for Event {
    fn borrow(&self) -> &Operation {
        &self.operation
    }
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
        self.subscription_tx.send(topic).await?;
        Ok(())
    }

    async fn initialize_topic_stream(
        &self,
        topic: TopicId,
    ) -> anyhow::Result<Option<Pin<Box<dyn Stream<Item = Operation> + Send + 'static>>>> {
        let Some(mailbox_rx) = self.mailboxes.subscribe(topic.into()).await? else {
            tracing::warn!("topic already iniitalized, skipping");
            return Ok(None);
        };

        let ingest: Ingest<SqliteStore, Event, LogId, Extensions, TopicId> =
            Ingest::new((*self.op_store).clone());

        let stream = ReceiverStream::new(mailbox_rx)
            .map(|op| {
                tracing::info!(topic = ?op.header.extensions.topic.renamed(), op = ?op.header.hash().renamed(), "received new operation from mailbox");
                let op = Operation::from(op);
                Event {
                    args: IngestArgs {
                        log_id: op.header.extensions.topic,
                        topic: op.header.extensions.topic,
                        prune_flag: false,
                    },
                    operation: op,
                }
            })
            .layer(ingest)
            .filter_map(|result| async {
                match result {
                    Ok((event, IngestResult::Inserted | IngestResult::AlreadyExists)) => {
                        Some(event.operation)
                    }
                    Err((event, err)) => {
                        tracing::error!(?event, ?err, "ingest error, op not ingested!");
                        None
                    }
                }
            });

        if let Some(tx) = &self.topic_subscribed_tx {
            let _ = tx.send(topic).await;
        }

        Ok(Some(Box::pin(stream)))
    }

    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all, fields(me=?self.device_id().renamed())))]
    pub(super) fn spawn_stream_process_loop(
        &self,
        mut subscription_rx: mpsc::Receiver<TopicId>,
        mut cancel_rx: mpsc::Receiver<()>,
    ) -> std::thread::JoinHandle<()> {
        let node = self.clone();

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime for current thread");

        let me = self.device_id();

        let handle = std::thread::spawn(move || {
            let local = tokio::task::LocalSet::new();

            local.spawn_local(
                async move {
                    let node = node.clone();
                    let mut streams = SelectAll::new();

                    loop {
                        tokio::select! {
                            Some(topic) = subscription_rx.recv() => {
                                match node.initialize_topic_stream(topic).await {
                                    Ok(Some(stream)) => {
                                        tracing::info!(topic = ?topic.renamed(), "subscribed to new topic");
                                        streams.push(stream);
                                    }
                                    Ok(None) => {
                                        tracing::info!("topic already initialized, skipping");
                                    }
                                    Err(err) => {
                                        tracing::error!(?err, "error initializing topic stream");
                                    }
                                }
                            }

                            Some(op) = streams.next() => {
                                tracing::info!(op = ?op.hash.renamed(), topic = ?op.header.extensions.topic.renamed(), "processing stream item");
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
                }
                .instrument(tracing::info_span!("stream_process_loop", me = ?me.renamed()))
            );

            rt.block_on(local);
            tracing::info!("stream processing loop finished");
        });

        handle
    }

    async fn process_stream_item(&self, operation: Operation) -> anyhow::Result<()> {
        let hash = operation.hash;
        let topic = operation.header.extensions.topic;

        let reordered = vec![operation];

        for operation in reordered {
            match self.process_operation(operation, false, false).await {
                Ok(()) => (),
                Err(err) => {
                    tracing::error!(
                        ?topic,
                        hash = ?hash.renamed(),
                        ?err,
                        "process operation error"
                    )
                }
            }
        }
        Ok(())
    }

    pub async fn process_operation(
        &self,
        // topic: Topic<K>,
        operation: Operation,
        is_author: bool,
        _is_repair: bool,
    ) -> anyhow::Result<()> {
        if let Err(err) = self.process_extensions(&operation).await {
            tracing::error!(?err, "process extensions error");
            return Err(err);
        }
        let Operation { header, body, hash } = operation;

        let topic = header.extensions.topic;

        // XXX: this eventually needs to be more selective than just adding any old author
        // author_store.add_author(topic, header.public_key).await;
        tracing::debug!(?topic, "adding author");

        tracing::debug!(topic = ?topic.renamed(), hash = ?hash.renamed(), "PROC: processing operation");

        let payload = body.map(|body| Payload::try_from_body(&body)).transpose()?;

        tracing::trace!(?payload, "RECEIVED PAYLOAD");

        // if !is_repair {
        if let Some(payload) = payload.as_ref() {
            if let Err(err) = self.process_payload(&header, payload, is_author).await {
                tracing::error!(
                    hash = ?header.hash().renamed(),
                    ?payload,
                    ?err,
                    "process operation error"
                );
                return Err(err);
            }
        }

        tracing::debug!(hash = ?hash.renamed(), "processed operation");

        if let Some(payload) = payload.as_ref() {
            self.notify_payload(&header, payload).await?;
        }

        // XXX: don't repair this often.
        // Box::pin(self.repair_spaces_and_publish()).await?;

        self.op_store.mark_op_processed(topic, &hash);

        anyhow::Ok(())
    }

    async fn process_extensions(&self, operation: &Operation) -> anyhow::Result<()> {
        match &operation.header.extensions.auth {
            Some(auth) => {
                tracing::info!(?auth, "processing auth extensions");
                if let Err(err) = self.group_store.process(operation).await {
                    tracing::error!(?err, "error processing auth extensions");
                };
                // Subscribe to announcements topics for any group members whose agent_id we know.
                let member_device_ids: Vec<DeviceId> = match &auth.action {
                    p2panda_auth::group::GroupAction::Create { initial_members } => initial_members
                        .iter()
                        .filter_map(|(m, _)| match m {
                            p2panda_auth::group::GroupMember::Individual(pk) => {
                                Some(DeviceId::from(*pk))
                            }
                            _ => None,
                        })
                        .collect(),
                    p2panda_auth::group::GroupAction::Add { member, .. } => match member {
                        p2panda_auth::group::GroupMember::Individual(pk) => {
                            vec![DeviceId::from(*pk)]
                        }
                        _ => vec![],
                    },
                    _ => vec![],
                };
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
            None => {}
        }
        Ok(())
    }

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

    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all, fields(me=?self.device_id().renamed())))]
    pub async fn process_payload(
        &self,
        // topic: Topic<K>,
        header: &Header,
        payload: &Payload,
        _is_author: bool,
    ) -> anyhow::Result<()> {
        let topic = header.extensions.topic;
        // TODO: maybe have different loops for the different kinds of topics and the different payloads in each
        match &payload {
            Payload::Chat(ChatPayload::JoinGroup { .. }) => {
                // Nothing to do.
            }

            Payload::Inbox(invitation) => {
                let active_topics = self.local_store.get_active_inbox_topics().await?;
                if !active_topics.iter().any(|it| **it.topic == *topic) {
                    // not for me, ignore
                    return Ok(());
                }
                tracing::info!(
                    ?invitation,
                    from = ?header.public_key.renamed(),
                    "received invitation message"
                );
                match invitation {
                    InboxPayload::ContactRequest { .. } => {
                        // Nothing to do.
                    }
                }
            }

            Payload::Chat(ChatPayload::Message(_) | ChatPayload::Reaction(_)) => {
                // Nothing to do.
            }

            Payload::Announcements(AnnouncementsPayload::SetCapabilities { .. }) => {
                // The announcements topic id IS the agent_id bytes, and the header public key is the device_id.
                // Save the device_id -> agent_id mapping so group members can look each other up.
                let device_id = DeviceId::from(header.public_key);
                let agent_id = AgentId::from(crate::ActorId::from_bytes(&*topic).map_err(|e| {
                    anyhow::anyhow!("invalid agent_id bytes in announcements topic: {e}")
                })?);
                if let Err(err) = self
                    .local_store
                    .save_agent_mapping(device_id, agent_id)
                    .await
                {
                    tracing::warn!(?err, "failed to save agent mapping from SetCapabilities");
                }
            }

            Payload::Announcements(_) => {
                // Nothing to do.
            }

            Payload::DeviceGroup(_) => {
                // Nothing to do.
            }
        }
        Ok(())
    }
}
