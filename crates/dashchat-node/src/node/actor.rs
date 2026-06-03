use std::collections::HashMap;
use std::pin::Pin;
use std::task::{Context, Poll};

use aliased::Aliasing;
use futures::future::join;
use futures::{FutureExt, Stream};
use p2panda::network::NetworkError;
use p2panda::node::CreateStreamError;
use p2panda::operation::{Extensions, LogId, Operation};
use p2panda::streams::{
    ExternalStreamFuture, ImportError, ProcessedOperation, PublishError, PublishFuture, Source,
    StreamEvent, StreamPublisher, StreamSubscription,
};
use p2panda::{Hash, NodeId, RelayUrl, Topic};
use thiserror::Error;
use tokio::select;
use tokio::sync::{mpsc, oneshot};
use tokio_stream::{StreamExt, StreamMap};
use tracing::warn;

use crate::Payload;
use crate::stores::GROUPS_STATE_ID;

type GroupsProcessor = p2panda_auth::processor::GroupsProcessor<Topic, Extensions, LogId>;

/// Node actor commands.
pub(crate) enum Command {
    Subscribe {
        topic: Topic,
        reply_tx: oneshot::Sender<Result<bool, NodeActorError>>,
    },
    #[allow(unused)]
    Unsubscribe {
        topic: Topic,
        reply_tx: oneshot::Sender<()>,
    },
    Import {
        topic: Topic,
        stream: Pin<Box<dyn Stream<Item = Operation> + Send>>,
        reply_tx: oneshot::Sender<Result<ExternalStreamFuture, NodeActorError>>,
    },
    Publish {
        topic: Topic,
        payload: Payload,
        reply_tx: oneshot::Sender<Result<ProcessFuture, NodeActorError>>,
    },
    RegisterBootstrap {
        node_id: NodeId,
        relay_url: RelayUrl,
        reply_tx: oneshot::Sender<Result<(), NodeActorError>>,
    },
    Shutdown {
        reply_tx: oneshot::Sender<()>,
    },
}

// Wrapper around StreamEvent from p2panda with variants for "system", "groups" and "application"
// events.
//
// This is used to express different variants of event types which will be forwarded to further
// application layer event processors and to package operations with their processed_tx and any
// errors which already occurred in this processor. The processed_tx is required so that the
// ProcessorFuture can be signaled to complete only after all application processing has occurred.
// The error is required so that if a groups control message fails processing, then the
// application layer can still decide separately whether to perform further processing or not.
//
// @TODO: This wrapping might not have been required if the node actor and app processing pipeline
// was combined into one process. I(sam) avoided doing that so as to keep my work as self
// contained as possible, it could be that i've generated some additional abstraction because of
// that though. It's also a side-effect of groups operations not being processed inside of the
// p2panda node yet, this generated some further error handling requirements. In any further
// refactoring it could be worth considering how these modules could actually be refactored into
// one place. In any case, it would be required to have both the processed_tx and additional error
// handling in place, so this is not wasted work in the long-run.
pub enum ProcessorEvent {
    System(StreamEvent<Payload>),
    Groups {
        operation: ProcessedOperation<Payload>,
        source: Source,
        processed_tx: Option<oneshot::Sender<Result<(), ProcessorError>>>,
        error: Option<ProcessorError>,
    },
    App {
        operation: ProcessedOperation<Payload>,
        source: Source,
        processed_tx: Option<oneshot::Sender<Result<(), ProcessorError>>>,
    },
}

/// Actor for the p2panda node.
///
/// This is a thin wrapper around the p2panda node API which includes merging of all subscription
/// streams and holding all publish handles. It also processes groups control messages when they
/// arrive on the stream and allows users to await this processing for operations they process
/// locally.
pub struct Actor {
    /// p2panda node.
    inner: p2panda::Node,

    /// All publishing channel senders.
    tx_map: HashMap<Topic, StreamPublisher<Payload>>,

    /// All subscription streams.
    streams: StreamMap<Topic, StreamSubscription<Payload>>,

    /// One shot channels for all received operations which resolve once the operation has
    /// completed additional processing.
    ///
    /// These are held while groups control messages are being processed so the user can await
    /// this processing on top of what the node already provides with PublishFuture. The oneshot
    /// channel sender is forwarded further up the processing pipeline (to the application layer)
    /// so that any further processing which occurs there can also be awaited.
    processed: HashMap<Hash, oneshot::Sender<Result<(), ProcessorError>>>,

    /// Groups processor.
    groups_processor: GroupsProcessor,

    /// Channel for forwarding all received events on to the application layer processor.
    events_tx: mpsc::Sender<ProcessorEvent>,
}

impl Actor {
    pub(crate) fn new(node: p2panda::Node) -> (Self, mpsc::Receiver<ProcessorEvent>) {
        let groups_processor = GroupsProcessor::new(node.store());
        let (events_tx, events_rx) = mpsc::channel(100);

        (
            Self {
                inner: node,
                tx_map: Default::default(),
                streams: Default::default(),
                processed: Default::default(),
                groups_processor,
                events_tx,
            },
            events_rx,
        )
    }

    pub(crate) async fn spawn(mut self) -> Result<mpsc::Sender<Command>, NodeActorError> {
        let (message_tx, mut message_rx) = mpsc::channel(100);

        let _ = tokio::spawn(async move {
            loop {
                select!(
                    Some(message) = message_rx.recv() => {
                        match message {
                            Command::Subscribe { topic, reply_tx } => {
                                let result = self.handle_subscribe(topic).await;
                                let _ = reply_tx.send(result);
                            }
                            Command::Unsubscribe { topic, reply_tx } => {
                                self.handle_unsubscribe(topic);
                                let _ = reply_tx.send(());
                            }
                            Command::Import { topic, stream, reply_tx } => {
                                let result = self.handle_import(topic, stream).await;
                                let _ = reply_tx.send(result);
                            }
                            Command::Publish {
                                topic,
                                payload,
                                reply_tx,
                            } => {
                                let result = self.handle_publish(topic, payload).await;
                                let _ = reply_tx.send(result);
                            }
                            Command::RegisterBootstrap { node_id, relay_url, reply_tx } => {
                                let result = self.handle_register_bootstrap(node_id, relay_url).await;
                                let _ = reply_tx.send(result);

                            },
                            Command::Shutdown { reply_tx } => {
                                // Drop self and then break out of the processing loop which will
                                // cause the actor task to complete.
                                drop(self);
                                let _ = reply_tx.send(());
                                break;
                            }
                        };
                    }
                    Some((_, event)) = self.streams.next() => {
                        let _ = self.process_event(event).await;
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

    async fn handle_subscribe(&mut self, topic: Topic) -> Result<bool, NodeActorError> {
        // If we're already subscribed to this topic then just return now.
        if self.tx_map.contains_key(&topic) {
            return Ok(false);
        }
        let (tx, rx) = self.inner.stream(topic).await?;
        self.tx_map.insert(topic, tx);
        self.streams.insert(topic, rx);
        Ok(true)
    }

    fn handle_unsubscribe(&mut self, topic: Topic) {
        self.tx_map.remove(&topic);
        self.streams.remove(&topic);
    }

    async fn handle_import(
        &mut self,
        topic: Topic,
        stream: Pin<Box<dyn Stream<Item = Operation> + Send>>,
    ) -> Result<ExternalStreamFuture, NodeActorError> {
        // Retrieve the topic_tx from the tx_map and if it isn't present subscribe to the topic.
        let tx = match self.tx_map.get(&topic) {
            Some(tx) => tx.clone(),
            None => {
                let (tx, rx) = self.inner.stream(topic).await?;
                self.tx_map.insert(topic, tx.clone());
                self.streams.insert(topic, rx);
                tx
            }
        };

        let import_fut = tx.import(stream).await?;
        Ok(import_fut)
    }

    async fn handle_publish(
        &mut self,
        topic: Topic,
        payload: Payload,
    ) -> Result<ProcessFuture, NodeActorError> {
        // Retrieve the topic_tx from the tx_map and if it isn't present subscribe to the topic.
        let tx = match self.tx_map.get(&topic) {
            Some(tx) => tx.clone(),
            None => {
                let (tx, rx) = self.inner.stream(topic).await?;
                self.tx_map.insert(topic, tx.clone());
                self.streams.insert(topic, rx);
                tx
            }
        };

        // If the payload represents a change to group state then publish it as a groups control
        // message, all other payload variants are published via the "normal" route.
        let publish_fut = match &payload {
            Payload::GroupControl(args) => tx.publish_groups(args.clone(), payload).await,
            _ => tx.publish(payload).await,
        }?;

        let (processed_tx, processed_rx) = oneshot::channel();
        let hash = publish_fut.hash();
        hash.alias_numbered();
        let _ = self.processed.insert(hash, processed_tx);
        let process_fut = ProcessFuture::new(hash, publish_fut, processed_rx);

        Ok(process_fut)
    }

    async fn handle_register_bootstrap(
        &self,
        node_id: NodeId,
        relay_url: RelayUrl,
    ) -> Result<(), NodeActorError> {
        self.inner.insert_bootstrap(node_id, relay_url).await?;
        Ok(())
    }

    async fn process_event(&mut self, event: StreamEvent<Payload>) -> Result<(), NodeActorError> {
        let processor_event = match &event {
            StreamEvent::Processed { operation, source } => {
                let id = operation.id();
                // For all processed operations remove the processed_tx from the map for forwarding to the
                // application layer.
                let processed_tx = self.processed.remove(&id);

                if let Payload::GroupControl(_) = operation.message() {
                    // Process any groups control messages.
                    let result = self.process_groups_control(operation).await;
                    if let Err(err) = result.as_ref() {
                        warn!("groups processing error: {err:?}");
                    }

                    ProcessorEvent::Groups {
                        operation: operation.clone(),
                        source: source.clone(),
                        processed_tx,
                        error: result.err(),
                    }
                } else {
                    ProcessorEvent::App {
                        operation: operation.clone(),
                        source: source.clone(),
                        processed_tx,
                    }
                }
            }
            _ => ProcessorEvent::System(event),
        };

        // Forward the event for further application layer processing.
        self.events_tx
            .send(processor_event)
            .await
            .map_err(|_| NodeActorError::EventSend)?;

        Ok(())
    }

    async fn process_groups_control(
        &self,
        operation: &ProcessedOperation<Payload>,
    ) -> Result<(), ProcessorError> {
        let topic = operation.topic();
        let header = operation.processed().header().to_owned();
        let body = operation.processed().body().cloned();

        let operation = Operation {
            hash: header.hash(),
            header,
            body,
        };

        self.groups_processor
            .process(&GROUPS_STATE_ID, &topic, &operation)
            .await
            .map_err(|err| ProcessorError::Groups(err.to_string()))?;

        Ok(())
    }
}

/// Future which can be awaited to find out when a locally published operation has finished
/// system and application layer processing.
pub struct ProcessFuture {
    hash: Hash,
    inner: Pin<Box<dyn Future<Output = <PublishFuture as Future>::Output> + Send + Sync>>,
}

impl ProcessFuture {
    pub fn new(
        hash: Hash,
        published_fut: PublishFuture,
        processed_rx: oneshot::Receiver<Result<(), ProcessorError>>,
    ) -> Self {
        Self {
            hash,
            inner: Box::pin(join(published_fut, processed_rx).map(|(result, _)| result)),
        }
    }
}

impl ProcessFuture {
    #[allow(unused)]
    pub fn hash(&self) -> Hash {
        self.hash
    }
}

impl Future for ProcessFuture {
    type Output = <PublishFuture as Future>::Output;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.inner.poll_unpin(cx)
    }
}

#[derive(Debug, Error)]
pub enum NodeActorError {
    #[error(transparent)]
    Publish(#[from] PublishError),

    #[error(transparent)]
    Subscribe(#[from] CreateStreamError),

    #[error(transparent)]
    Import(#[from] ImportError),

    #[error("error sending on event tx")]
    EventSend,

    #[error(transparent)]
    Network(#[from] NetworkError),
}

#[derive(Clone, Debug, Error)]
pub enum ProcessorError {
    #[error("application layer processing error: {0}")]
    App(String),

    #[error("groups operation processing error: {0}")]
    Groups(String),
}

#[cfg(test)]
mod tests {
    use futures::future::join_all;
    use p2panda::{Hash, Node, SigningKey, Topic, VerifyingKey};
    use p2panda_auth::Access;
    use p2panda_auth::group::{GroupAction, GroupCrdtState, GroupMember};
    use p2panda_auth::processor::{GroupsArgs, GroupsOperation};
    use p2panda_store::groups::GroupsStore;
    use p2panda_store::{SqliteStore, tx_unwrap};
    use tokio::sync::oneshot;

    use crate::node::actor::ProcessorEvent;
    use crate::testing::setup_tracing;
    use crate::{ChatMessageContent, ChatPayload, Payload};

    use super::{Actor, Command};

    type GroupsState = GroupCrdtState<VerifyingKey, Hash, GroupsOperation, ()>;

    fn chat(message: &str) -> Payload {
        Payload::Chat(ChatPayload::Message(ChatMessageContent::text_only(message)))
    }

    async fn groups_control(
        store: &SqliteStore,
        group_id: VerifyingKey,
        action: GroupAction<VerifyingKey>,
    ) -> Payload {
        let groups_y: GroupsState = tx_unwrap!(store, { store.get_groups_state(&0).await })
            .unwrap()
            .unwrap_or_default();

        let dependencies = groups_y.heads();
        Payload::GroupControl(GroupsArgs {
            group_id,
            action,
            dependencies,
        })
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn subscribe_and_send() {
        setup_tracing(&["dashchat=info"], true);

        let network_id = Topic::random();

        let topic_a = Topic::random();
        let topic_b = Topic::random();

        let alice = Node::builder()
            .network_id(network_id.into())
            .spawn()
            .await
            .unwrap();
        let bobbi = Node::builder()
            .network_id(network_id.into())
            .spawn()
            .await
            .unwrap();

        let (alice_actor, alice_events_rx) = Actor::new(alice);
        let alice_actor_tx = alice_actor.spawn().await.unwrap();

        let (bobbi_actor, bobbi_events_rx) = Actor::new(bobbi);
        let bobbi_actor_tx = bobbi_actor.spawn().await.unwrap();

        // Both alice and bobbi subscribe to topics a & b.
        for topic in [topic_a, topic_b] {
            let (reply_tx, reply_rx) = oneshot::channel();
            alice_actor_tx
                .send(Command::Subscribe { topic, reply_tx })
                .await
                .unwrap();

            assert!(reply_rx.await.unwrap().unwrap());

            let (reply_tx, reply_rx) = oneshot::channel();
            bobbi_actor_tx
                .send(Command::Subscribe { topic, reply_tx })
                .await
                .unwrap();

            assert!(reply_rx.await.unwrap().unwrap());
        }

        // Alice sends a message into each topic.
        let topic_a_message = chat("hey from topic a!");
        let topic_b_message = chat("hey from topic b!");

        let mut processed_futures = vec![];
        for (topic, payload) in [
            (topic_a, topic_a_message.clone()),
            (topic_b, topic_b_message.clone()),
        ] {
            let (reply_tx, reply_rx) = oneshot::channel();
            alice_actor_tx
                .send(Command::Publish {
                    topic,
                    payload,
                    reply_tx,
                })
                .await
                .unwrap();

            let processed_future = reply_rx.await.unwrap().unwrap();
            processed_futures.push(processed_future);
        }

        // Both alice and bobbi receive the messages on their events stream.
        for mut events_rx in [alice_events_rx, bobbi_events_rx] {
            let mut topic_a_message_received = false;
            let mut topic_b_message_received = false;
            while let Some(ProcessorEvent::App { operation, .. }) = events_rx.recv().await {
                if operation.message() == &topic_a_message {
                    topic_a_message_received = true;
                }

                if operation.message() == &topic_b_message {
                    topic_b_message_received = true;
                }

                if topic_a_message_received && topic_b_message_received {
                    break;
                }
            }
        }

        for event in join_all(processed_futures).await {
            assert!(event.unwrap().is_completed());
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn process_groups_control_messages() {
        setup_tracing(&["dashchat=info", "named_id=warn"], true);

        let network_id = Topic::random();
        let topic = Topic::random();

        let alice = Node::builder()
            .network_id(network_id.into())
            .spawn()
            .await
            .unwrap();
        let bobbi = Node::builder()
            .network_id(network_id.into())
            .spawn()
            .await
            .unwrap();

        let alice_store = alice.store();
        let bobbi_store = bobbi.store();
        let alice_id = alice.id();
        let bobbi_id = bobbi.id();

        let (alice_actor, alice_events_rx) = Actor::new(alice);
        let alice_actor_tx = alice_actor.spawn().await.unwrap();

        let (bobbi_actor, bobbi_events_rx) = Actor::new(bobbi);
        let bobbi_actor_tx = bobbi_actor.spawn().await.unwrap();

        // Alice subscribes to topic.
        let (reply_tx, reply_rx) = oneshot::channel();
        alice_actor_tx
            .send(Command::Subscribe { topic, reply_tx })
            .await
            .unwrap();
        assert!(reply_rx.await.unwrap().unwrap());

        // Bobbi subscribes to topic.
        let (reply_tx, reply_rx) = oneshot::channel();
        bobbi_actor_tx
            .send(Command::Subscribe { topic, reply_tx })
            .await
            .unwrap();
        assert!(reply_rx.await.unwrap().unwrap());

        // Alice publishes a "create" group message.
        let group_id = SigningKey::generate().verifying_key();
        let create_group = groups_control(
            &alice_store,
            group_id,
            GroupAction::Create {
                initial_members: vec![
                    (GroupMember::Individual(alice_id), Access::manage()),
                    (GroupMember::Individual(bobbi_id), Access::manage()),
                ],
            },
        )
        .await;

        let (reply_tx, reply_rx) = oneshot::channel();
        alice_actor_tx
            .send(Command::Publish {
                topic,
                payload: create_group.clone(),
                reply_tx,
            })
            .await
            .unwrap();
        let processed_fut = reply_rx.await.unwrap().unwrap();

        // Both receive the message on their events stream.
        for mut events_rx in [alice_events_rx, bobbi_events_rx] {
            while let Some(event) = events_rx.recv().await {
                if let ProcessorEvent::Groups { operation, .. } = event {
                    if operation.message() == &create_group {
                        break;
                    }
                }
            }
        }

        assert!(processed_fut.await.unwrap().is_completed());

        // And they have also processed the groups control message.
        for store in [alice_store, bobbi_store] {
            let groups_y: GroupsState = tx_unwrap!(store, { store.get_groups_state(&0).await })
                .unwrap()
                .unwrap();
            let members = groups_y.members(group_id);
            assert!(members.contains(&(alice_id, Access::manage())));
            assert!(members.contains(&(bobbi_id, Access::manage())));
        }
    }
}
