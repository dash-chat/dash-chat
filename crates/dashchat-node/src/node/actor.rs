use std::collections::HashMap;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures::{FutureExt, Stream};
use p2panda::operation::{Extensions, LogId, Operation};
use p2panda::streams::{
    ExternalStreamFuture, PublishError, PublishFuture, StreamEvent, StreamPublisher,
    StreamSubscription,
};
use p2panda::{Hash, Topic};
use p2panda_auth::processor::GroupsArgs;
use thiserror::Error;
use tokio::select;
use tokio::sync::{mpsc, oneshot};
use tokio_stream::{StreamExt, StreamMap};
use tracing::warn;

use crate::Payload;

type GroupsProcessor = p2panda_auth::processor::GroupsProcessor<Topic, Extensions, LogId>;

pub(crate) enum Command {
    Subscribe {
        topic: Topic,
        reply_tx: oneshot::Sender<()>,
    },
    Unsubscribe {
        topic: Topic,
        reply_tx: oneshot::Sender<()>,
    },
    Import {
        topic: Topic,
        stream: Pin<Box<dyn Stream<Item = Operation> + Send>>,
        reply_tx: oneshot::Sender<ExternalStreamFuture>,
    },
    Publish {
        topic: Topic,
        payload: Payload,
        reply_tx: oneshot::Sender<Result<(PublishFuture, ProcessFuture), NodeActorError>>,
    },
    Shutdown {
        reply_tx: oneshot::Sender<()>,
    },
}

/// Future which can be awaited to find out when a locally published operation has finished
/// application layer processing.
#[derive(Debug)]
pub struct ProcessFuture {
    hash: Hash,
    processed_rx: oneshot::Receiver<()>,
}

impl ProcessFuture {
    /// Returns hash of the published operation.
    pub fn hash(&self) -> Hash {
        self.hash
    }
}

impl Future for ProcessFuture {
    type Output = Result<(), oneshot::error::RecvError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.processed_rx.poll_unpin(cx)
    }
}

pub struct Actor {
    inner: p2panda::Node,
    tx_map: HashMap<Topic, StreamPublisher<Payload>>,
    processed: HashMap<Hash, oneshot::Sender<()>>,
    streams: StreamMap<Topic, StreamSubscription<Payload>>,
}

impl Actor {
    pub(crate) fn new(node: p2panda::Node) -> Self {
        Self {
            inner: node,
            tx_map: Default::default(),
            streams: Default::default(),
            processed: Default::default(),
        }
    }

    pub(crate) async fn spawn(mut self) -> Result<mpsc::Sender<Command>, NodeActorError> {
        let (message_tx, mut message_rx) = mpsc::channel(100);

        let _ = tokio::spawn(async move {
            loop {
                select!(
                    Some(message) = message_rx.recv() => {
                        match message {
                            Command::Subscribe { topic, reply_tx } => {
                                self.handle_subscribe(topic).await;
                                let _ = reply_tx.send(());
                            }
                            Command::Unsubscribe { topic, reply_tx } => {
                                self.handle_unsubscribe(topic).await;
                                let _ = reply_tx.send(());
                            }
                            Command::Import { topic, stream, reply_tx } => {
                                let done_fut = self.handle_import(topic, stream).await;
                                let _ = reply_tx.send(done_fut);
                            }
                            Command::Publish {
                                topic,
                                payload,
                                reply_tx,
                            } => {
                                let done_fut = self.handle_publish(topic, payload).await;
                                let _ = reply_tx.send(done_fut);
                            }
                            Command::Shutdown { reply_tx } => {
                                self.handle_shutdown().await;
                                let _ = reply_tx.send(());
                            }
                        };
                    }
                    Some((topic, event)) = self.streams.next() => {
                        let _ = self.process_event(topic, event).await;
                    }
                    else => {
                        warn!("node actor message channel closed, exiting event loop");
                        break;
                    }
                );
            }
        });

        Ok(message_tx)
    }

    async fn handle_subscribe(&self, topic: Topic) {}
    async fn handle_unsubscribe(&self, topic: Topic) {}
    async fn handle_import(
        &self,
        topic: Topic,
        stream: Pin<Box<dyn Stream<Item = Operation> + Send>>,
    ) -> ExternalStreamFuture {
        todo!()
    }
    async fn handle_publish(
        &mut self,
        topic: Topic,
        payload: Payload,
    ) -> Result<(PublishFuture, ProcessFuture), NodeActorError> {
        // Retrieve the topic tx from the tx_map. If it isn't present it means we didn't subscribe
        // to this topic yet.
        let Some(tx) = self.tx_map.get(&topic) else {
            return Err(NodeActorError::MissingTopicTX(topic));
        };

        // If the payload represents a change to group state then publish it as a groups control
        // message, all other payload variants are published via the "normal" route. 
        let publish_fut = match &payload {
            Payload::GroupControl(args) => tx.publish_groups(args.clone(), payload).await,
            _ => tx.publish(payload).await,
        }?;

        let (processed_tx, processed_rx) = oneshot::channel();
        let hash = publish_fut.hash();
        let _ = self.processed.insert(hash, processed_tx);
        let process_fut = ProcessFuture { hash, processed_rx };
        Ok((publish_fut, process_fut))
    }
    async fn handle_shutdown(&self) {}
    async fn process_event(&mut self, topic: Topic, event: StreamEvent<Payload>) {
        match event {
            StreamEvent::Processed { operation, source } => {
                if let Some(processed_tx) = self.processed.remove(&operation.id()) {
                    let _ = processed_tx.send(());
                }
            }
            StreamEvent::SyncStarted { .. } => (),
            StreamEvent::SyncEnded { .. } => (),
            StreamEvent::ImportStarted { session_id } => (),
            StreamEvent::ImportEnded { session_id } => (),
            StreamEvent::ProcessingFailed { .. } => (),
            StreamEvent::DecodeFailed { event, error } => (),
            StreamEvent::ReplayFailed { error } => (),
            StreamEvent::AckFailed { event, error } => (),
        }
    }

    //     // @TODO: I removed a boolean argument from this method which signified if this was an
    //     // operation created by the local author. I couldn't see where it was needed. If it needs
    //     // to be brought back we can check if we authored the operation internally here.
    //     pub async fn process_operation(
    //         &self,
    //         operation: ProcessedOperation<Payload>,
    //     ) -> anyhow::Result<()> {
    //         let topic = operation.topic();
    //         let hash = operation.id();
    //         let author = operation.author();
    //         let payload = operation.message();
    //
    //         match payload {
    //             Payload::Chat(ChatPayload::JoinGroup { .. }) => {
    //                 // Nothing to do.
    //             }
    //
    //             Payload::Inbox(invitation) => {
    //                 // @TODO(sam): what is the purpose of this active topic check? Intuitively I would
    //                 // assume that the topics we are currently subscribed to are our "active topics"
    //                 // or am I missing something important?
    //                 let active_topics = self.local_store.get_active_inbox_topics().await?;
    //                 if !active_topics
    //                     .iter()
    //                     .any(|it| P2PandaTopic::from(it.topic) == topic)
    //                 {
    //                     // not for me, ignore
    //                     return Ok(());
    //                 }
    //                 tracing::info!(
    //                     invitation = ?invitation.renamed_ref(),
    //                     from = ?author,
    //                     "received invitation message"
    //                 );
    //                 match invitation {
    //                     InboxPayload::ContactRequest { .. } => {
    //                         // Nothing to do.
    //                     }
    //                 }
    //             }
    //
    //             Payload::Chat(ChatPayload::Message(_) | ChatPayload::Reaction(_)) => {
    //                 // Nothing to do.
    //             }
    //
    //             Payload::Announcements(AnnouncementsPayload::SetProfile(profile)) => {
    //                 // HACK: The announcements topic id IS the agent_id bytes, so we can reconstruct it here.
    //                 let agent_id =
    //                     AgentId::from(crate::ActorId::from_bytes(topic.as_bytes()).map_err(|e| {
    //                         anyhow::anyhow!("invalid agent_id bytes in announcements topic: {e}")
    //                     })?);
    //
    //                 if let Err(err) = self
    //                     .local_store
    //                     .save_profile(agent_id, profile.clone())
    //                     .await
    //                 {
    //                     tracing::warn!(?err, "failed to save profile from SetProfile");
    //                 }
    //             }
    //
    //             Payload::Announcements(AnnouncementsPayload::SetCapabilities { capabilities }) => {
    //                 // Save the device_id -> agent_id mapping so group members can look each other up.
    //
    //                 let device_id = DeviceId::from(author);
    //                 // HACK: The announcements topic id IS the agent_id bytes, so we can reconstruct it here.
    //                 let agent_id =
    //                     AgentId::from(crate::ActorId::from_bytes(topic.as_bytes()).map_err(|e| {
    //                         anyhow::anyhow!("invalid agent_id bytes in announcements topic: {e}")
    //                     })?);
    //                 if let Err(err) = self
    //                     .local_store
    //                     .save_agent_mapping(device_id, agent_id)
    //                     .await
    //                 {
    //                     tracing::warn!(?err, "failed to save agent mapping from SetCapabilities");
    //                 }
    //
    //                 if let Err(err) = self
    //                     .local_store
    //                     .save_capabilities(device_id, capabilities.clone())
    //                     .await
    //                 {
    //                     tracing::warn!(?err, "failed to save capabilities from SetCapabilities");
    //                 }
    //             }
    //
    //             Payload::DeviceGroup(_) => {
    //                 // Nothing to do.
    //             }
    //             Payload::GroupsControl => {
    //                 // Process the groups control message. The payload is just a placeholder for now
    //                 // so that we can easily identify these messages.
    //                 self.process_groups_control(topic, &operation).await?;
    //             }
    //         }
    //
    //         tracing::debug!(hash = ?hash, "processed operation");
    //
    //         // For all message types except groups control messages notify that a new payload has been
    //         // received.
    //         if let Payload::GroupsControl = payload {
    //             self.notify_payload(&operation.processed().header(), payload)
    //                 .await?;
    //         }
    //
    //         // @TODO(sam): clarify if this is needed, I can't see where exactly it's used.
    //         self.op_store
    //             .mark_op_processed(topic.to_bytes().into(), &hash);
    //
    //         anyhow::Ok(())
    //     }
    //
    //     async fn process_groups_control(
    //         &self,
    //         topic: P2PandaTopic,
    //         operation: &ProcessedOperation<Payload>,
    //     ) -> anyhow::Result<()> {
    //         let header = operation.processed().header().to_owned();
    //         let body = operation.processed().body().cloned();
    //
    //         match header.extensions.groups_args.clone() {
    //             Some(auth) => {
    //                 let operation = Operation {
    //                     hash: header.hash(),
    //                     header,
    //                     body,
    //                 };
    //                 if let Err(err) = self.group_store.process(topic, &operation).await {
    //                     tracing::error!(?err, "error processing auth extensions");
    //                 };
    //                 // Subscribe to announcements topics for any group members whose agent_id we know.
    //                 let member_device_ids: Vec<DeviceId> = match &auth.action {
    //                     p2panda_auth::group::GroupAction::Create { initial_members } => initial_members
    //                         .iter()
    //                         .filter_map(|(m, _)| match m {
    //                             p2panda_auth::group::GroupMember::Individual(pk) => {
    //                                 Some(DeviceId::from(*pk))
    //                             }
    //                             _ => None,
    //                         })
    //                         .collect(),
    //                     p2panda_auth::group::GroupAction::Add { member, .. } => match member {
    //                         p2panda_auth::group::GroupMember::Individual(pk) => {
    //                             vec![DeviceId::from(*pk)]
    //                         }
    //                         _ => vec![],
    //                     },
    //                     _ => vec![],
    //                 };
    //                 let known = self
    //                     .local_store
    //                     .lookup_contacts(member_device_ids.iter())
    //                     .await?;
    //                 for agent_id in known.into_values() {
    //                     let topic = Topic::announcements(agent_id);
    //                     if let Err(err) = self.initialize_topic(*topic).await {
    //                         tracing::warn!(
    //                             ?err,
    //                             "failed to subscribe to announcements topic for group member"
    //                         );
    //                     }
    //                 }
    //             }
    //             None => {}
    //         }
    //         Ok(())
    //     }
    //
    //     pub async fn notify_payload(&self, header: &Header, payload: &Payload) -> anyhow::Result<()> {
    //         if let Some((notification_tx, payload)) = self.notification_tx.clone().zip(Some(payload)) {
    //             notification_tx
    //                 .send(Notification {
    //                     header: header.clone(),
    //                     payload: payload.clone(),
    //                 })
    //                 .await
    //                 .unwrap_or_else(|_| tracing::warn!("notification channel closed"));
    //         }
    //         Ok(())
    //     }
}

#[derive(Debug, Error)]
pub enum NodeActorError {
    #[error("missing publish stream for topic: {0}")]
    MissingTopicTX(Topic),

    #[error(transparent)]
    Publish(#[from] PublishError),
}
