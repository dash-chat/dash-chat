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
    ExternalStreamFuture, ImportError, ProcessedOperation, PublishError, PublishFuture,
    StreamEvent, StreamPublisher, StreamSubscription,
};
use p2panda::{Hash, NodeId, RelayUrl, Topic};
use p2panda_auth::processor::GroupsProcessorError;
use thiserror::Error;
use tokio::select;
use tokio::sync::{broadcast, mpsc, oneshot};
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
    /// this processing on top of what the node already provides with PublishFuture.
    ///
    /// // @TODO: could be good to allow the user to await further dash chat application layer
    /// processing with this same mechanism, this would be possible if we forward the tx on along
    /// with the operation onto the application layer.
    processed: HashMap<Hash, oneshot::Sender<()>>,

    /// Groups processor.
    groups_processor: GroupsProcessor,

    /// Channel for forwarding all received events on to the application layer processor.
    events_tx: broadcast::Sender<StreamEvent<Payload>>,
}

impl Actor {
    pub(crate) fn new(node: p2panda::Node) -> (Self, broadcast::Receiver<StreamEvent<Payload>>) {
        let groups_processor = GroupsProcessor::new(node.store());
        let (events_tx, events_rx) = broadcast::channel(100);

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
        &self,
        topic: Topic,
        stream: Pin<Box<dyn Stream<Item = Operation> + Send>>,
    ) -> Result<ExternalStreamFuture, NodeActorError> {
        // Retrieve the topic tx from the tx_map. If it isn't present it means we didn't subscribe
        // to this topic yet.
        let Some(tx) = self.tx_map.get(&topic) else {
            return Err(NodeActorError::MissingTopicTX(topic));
        };

        let import_fut = tx.import(stream).await?;
        Ok(import_fut)
    }

    async fn handle_publish(
        &mut self,
        topic: Topic,
        payload: Payload,
    ) -> Result<ProcessFuture, NodeActorError> {
        // @TODO: from running the tests I can see there are some places where we are trying to
        // publish an operation before subscribing to the topic. Ideally we error if there is no
        // existing stream for the topic we are publishing into. I've added a logic here to
        // automatically subscribe to a topic if the tx is not present. We should rather figure
        // out where calls to subscribe to the topic is missing and remove this snippet.
        if !self.tx_map.contains_key(&topic) {
            let (tx, rx) = self.inner.stream(topic).await?;
            self.tx_map.insert(topic, tx);
            self.streams.insert(topic, rx);
        }

        // Retrieve the topic tx from the tx_map.
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
        if let StreamEvent::Processed { operation, .. } = &event {
            let id = operation.id();

            // Process any group control messages.
            if let Payload::GroupControl(_) = operation.message() {
                self.process_groups_control(operation).await?;
            }

            // Signal that the event has been processed at system level.
            if let Some(processed_tx) = self.processed.remove(&id) {
                let _ = processed_tx.send(());
            };
        }

        // Forward the event for further application layer processing.
        self.events_tx
            .send(event)
            .map_err(|_| NodeActorError::EventSend)?;

        Ok(())
    }

    async fn process_groups_control(
        &self,
        operation: &ProcessedOperation<Payload>,
    ) -> Result<(), NodeActorError> {
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
            .await?;

        Ok(())
    }
}

/// Future which can be awaited to find out when a locally published operation has finished
/// system layer processing.
pub struct ProcessFuture {
    hash: Hash,
    inner: Pin<Box<dyn Future<Output = <PublishFuture as Future>::Output> + Send + Sync>>,
}

impl ProcessFuture {
    pub fn new(
        hash: Hash,
        published_fut: PublishFuture,
        processed_rx: oneshot::Receiver<()>,
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
    #[error("missing publish stream for topic: {0}")]
    MissingTopicTX(Topic),

    #[error(transparent)]
    Publish(#[from] PublishError),

    #[error(transparent)]
    Subscribe(#[from] CreateStreamError),

    #[error(transparent)]
    Import(#[from] ImportError),

    #[error(transparent)]
    GroupsProcessor(#[from] GroupsProcessorError<()>),

    #[error("error sending on event tx")]
    EventSend,

    #[error(transparent)]
    Network(#[from] NetworkError),
}

#[cfg(test)]
mod tests {
    use p2panda::streams::StreamEvent;
    use p2panda::{Hash, SigningKey, Topic, VerifyingKey};
    use p2panda_auth::Access;
    use p2panda_auth::group::{GroupAction, GroupCrdtState, GroupMember};
    use p2panda_auth::processor::{GroupsArgs, GroupsOperation};
    use p2panda_store::groups::GroupsStore;
    use p2panda_store::{SqliteStore, tx_unwrap};
    use tokio::sync::oneshot;

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

        let topic_a = Topic::random();
        let topic_b = Topic::random();

        let alice = p2panda::spawn().await.unwrap();
        let bobbi = p2panda::spawn().await.unwrap();

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

            let event = reply_rx.await.unwrap().unwrap().await.unwrap();
            assert!(event.is_completed());
        }

        // Both alice and bobbi receive the messages on their events stream.
        for mut events_rx in [alice_events_rx, bobbi_events_rx] {
            let mut topic_a_message_received = false;
            let mut topic_b_message_received = false;
            while let Ok(event) = events_rx.recv().await {
                if let StreamEvent::Processed { operation, .. } = event {
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
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn process_groups_control_messages() {
        setup_tracing(&["dashchat=info"], true);

        let topic = Topic::random();

        let alice = p2panda::spawn().await.unwrap();
        let bobbi = p2panda::spawn().await.unwrap();
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

        let event = reply_rx.await.unwrap().unwrap().await.unwrap();
        assert!(event.is_completed());

        // Alice's has processed the groups message.
        let groups_y: GroupsState =
            tx_unwrap!(alice_store, { alice_store.get_groups_state(&0).await })
                .unwrap()
                .unwrap();
        let members = groups_y.members(group_id);
        assert!(members.contains(&(alice_id, Access::manage())));
        assert!(members.contains(&(bobbi_id, Access::manage())));

        // Bobbi receives the messages on their events stream.
        for mut events_rx in [alice_events_rx, bobbi_events_rx] {
            while let Ok(event) = events_rx.recv().await {
                if let StreamEvent::Processed { operation, .. } = event {
                    if operation.message() == &create_group {
                        break;
                    }
                }
            }
        }

        // And they have also processed the groups control message.
        let groups_y: GroupsState =
            tx_unwrap!(bobbi_store, { bobbi_store.get_groups_state(&0).await })
                .unwrap()
                .unwrap();
        let members = groups_y.members(group_id);
        assert!(members.contains(&(alice_id, Access::manage())));
        assert!(members.contains(&(bobbi_id, Access::manage())));
    }
}
