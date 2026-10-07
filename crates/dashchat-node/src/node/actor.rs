use std::collections::{HashMap, VecDeque};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use aliased::Aliasing;
use futures::future::join;
use futures::{FutureExt, Stream};
use p2panda::network::NetworkError;
use p2panda::node::CreateStreamError;
use p2panda::operation::Operation;
use p2panda::streams::{
    ImportError, ProcessedOperation, PublishError, PublishFuture, Source, StreamEvent, StreamFrom,
    StreamPublisher, StreamSubscription,
};
use p2panda::{Hash, NodeId, RelayUrl, Topic};
use thiserror::Error;
use tokio::select;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinSet;
use tokio_stream::{StreamExt, StreamMap};
use tracing::{error, warn};

use crate::Payload;

pub(crate) type ProcessedTx = oneshot::Sender<Result<(), ProcessorError>>;
type ProcessedRx = oneshot::Receiver<Result<(), ProcessorError>>;

/// Own-authored operations the drain task forwarded before `publish` claimed them (see
/// [`Processed`]). The window is the gap between p2panda returning the hash and the claim, so
/// this only ever holds a handful; the cap just guards against operations we authored through
/// other routes (replays, local imports) and never claim.
const UNCLAIMED_CAPACITY: usize = 1024;

/// Something [`Drain::next`] hands to the application processor.
pub enum ProcessorEvent {
    /// An operation ready for application processing. `processed_tx` is set when a local
    /// `publish` is awaiting it, and must be resolved once processing is done.
    Operation {
        operation: ProcessedOperation<Payload>,
        source: Source,
        processed_tx: Option<ProcessedTx>,
    },

    ImportFailed {
        topic: Topic,
        error: ImportError,
    },
}

/// Per-topic p2panda streams.
///
/// This is a thin wrapper around the p2panda node API which holds all publish handles. Every
/// subscription stream is merged into the [`Drain`], which the application processor pulls
/// operations from, and callers can await that processing for operations they published locally.
///
/// Publishing and subscribing are plain methods, so calls never queue behind event processing the
/// way an actor command would.
#[derive(Clone)]
pub struct Streams {
    inner: Arc<Inner>,
}

struct Inner {
    /// p2panda node.
    node: p2panda::Node,

    /// Prefix for each topic stream's ack cursor name. `None` uses p2panda's
    /// default per-topic cursor (`"{topic}"`);
    stream_cursor_prefix: Option<String>,

    /// All publishing channel senders.
    ///
    /// Held across opening a new topic's stream so two callers racing on the same new topic
    /// can't open it twice.
    publishers: tokio::sync::Mutex<HashMap<Topic, StreamPublisher<Payload>>>,

    processed: Arc<std::sync::Mutex<Processed>>,

    drain_tx: mpsc::UnboundedSender<DrainCommand>,
}

/// One shot channels for locally published operations which resolve once the operation has
/// completed application processing, on top of what the node already provides with
/// PublishFuture.
///
/// p2panda hands an operation to the pipeline before `publish` learns its hash, so the drain can
/// yield it before the publisher registers interest. For own-authored operations the drain then
/// attaches a sender of its own and parks the receiver in `unclaimed` for `publish` to pick up.
#[derive(Default)]
struct Processed {
    registered: HashMap<Hash, ProcessedTx>,
    unclaimed: HashMap<Hash, ProcessedRx>,
    unclaimed_order: VecDeque<Hash>,
}

impl Processed {
    /// Sender to attach to a processed operation's event, if anyone will await it.
    fn take_sender(&mut self, hash: Hash, authored_by_us: bool) -> Option<ProcessedTx> {
        if let Some(tx) = self.registered.remove(&hash) {
            return Some(tx);
        }
        if !authored_by_us {
            return None;
        }
        let (tx, rx) = oneshot::channel();
        self.unclaimed.insert(hash, rx);
        self.unclaimed_order.push_back(hash);
        while self.unclaimed_order.len() > UNCLAIMED_CAPACITY {
            if let Some(evicted) = self.unclaimed_order.pop_front() {
                self.unclaimed.remove(&evicted);
            }
        }
        Some(tx)
    }

    /// Receiver `publish` awaits for an operation it just published.
    fn claim(&mut self, hash: Hash) -> ProcessedRx {
        if let Some(rx) = self.unclaimed.remove(&hash) {
            return rx;
        }
        let (tx, rx) = oneshot::channel();
        self.registered.insert(hash, tx);
        rx
    }
}

enum DrainCommand {
    Subscribe(Topic, StreamSubscription<Payload>),
    Unsubscribe(Topic),
    Import {
        topic: Topic,
        publisher: StreamPublisher<Payload>,
        stream: Pin<Box<dyn Stream<Item = Operation> + Send>>,
    },
}

impl Streams {
    pub(crate) fn new(node: p2panda::Node, stream_cursor_prefix: Option<String>) -> (Self, Drain) {
        let (drain_tx, command_rx) = mpsc::unbounded_channel();
        let processed = Arc::new(std::sync::Mutex::new(Processed::default()));

        let drain = Drain {
            node_id: node.id(),
            streams: Default::default(),
            processed: processed.clone(),
            command_rx,
            import_tasks: JoinSet::new(),
        };

        let streams = Self {
            inner: Arc::new(Inner {
                node,
                stream_cursor_prefix,
                publishers: Default::default(),
                processed,
                drain_tx,
            }),
        };

        (streams, drain)
    }

    /// Open a topic stream, tracking its ack cursor under a per-topic name. With
    /// a cursor prefix set (the iOS push extension) the name is
    /// `"{prefix}:{topic}"`, so each topic keeps its own cursor while staying
    /// distinct from the app's default `"{topic}"` cursor: the two processes
    /// share one database and must not advance each other's cursors.
    async fn open_stream(
        &self,
        topic: Topic,
    ) -> Result<(StreamPublisher<Payload>, StreamSubscription<Payload>), CreateStreamError> {
        let cursor_name = self
            .inner
            .stream_cursor_prefix
            .as_ref()
            .map(|prefix| format!("{prefix}:{topic}"));
        self.inner
            .node
            .stream_from(topic, StreamFrom::Frontier, cursor_name)
            .await
    }

    /// The topic's publisher, opening its stream if this is the first use of the topic.
    ///
    /// Returns whether the stream was newly opened.
    async fn publisher(
        &self,
        topic: Topic,
    ) -> Result<(StreamPublisher<Payload>, bool), StreamsError> {
        let mut publishers = self.inner.publishers.lock().await;
        if let Some(tx) = publishers.get(&topic) {
            return Ok((tx.clone(), false));
        }
        let (tx, rx) = self.open_stream(topic).await?;
        publishers.insert(topic, tx.clone());
        self.send_to_drain(DrainCommand::Subscribe(topic, rx))?;
        Ok((tx, true))
    }

    fn send_to_drain(&self, command: DrainCommand) -> Result<(), StreamsError> {
        self.inner
            .drain_tx
            .send(command)
            .map_err(|_| StreamsError::DrainClosed)
    }

    /// Subscribe to a topic. Returns `false` if already subscribed.
    pub(crate) async fn subscribe(&self, topic: Topic) -> Result<bool, StreamsError> {
        let (_, opened) = self.publisher(topic).await?;
        Ok(opened)
    }

    #[allow(unused)]
    pub(crate) async fn unsubscribe(&self, topic: Topic) {
        self.inner.publishers.lock().await.remove(&topic);
        let _ = self.send_to_drain(DrainCommand::Unsubscribe(topic));
    }

    /// Import an external stream of operations into a topic, subscribing to it if needed.
    pub(crate) async fn import(
        &self,
        topic: Topic,
        stream: Pin<Box<dyn Stream<Item = Operation> + Send>>,
    ) -> Result<(), StreamsError> {
        let (publisher, _) = self.publisher(topic).await?;
        self.send_to_drain(DrainCommand::Import {
            topic,
            publisher,
            stream,
        })
    }

    /// Publish a payload into a topic, subscribing to it if needed.
    pub(crate) async fn publish(
        &self,
        topic: Topic,
        payload: Payload,
    ) -> Result<ProcessFuture, StreamsError> {
        let (tx, _) = self.publisher(topic).await?;

        // If the payload represents a change to group state then publish it as a groups control
        // message, all other payload variants are published via the "normal" route.
        let publish_fut = tx.publish(payload).await?;

        let hash = publish_fut.hash();
        hash.alias_numbered();
        let processed_rx = self.inner.processed.lock().unwrap().claim(hash);

        Ok(ProcessFuture::new(hash, publish_fut, processed_rx))
    }

    pub(crate) async fn register_bootstrap(
        &self,
        node_id: NodeId,
        relay_url: RelayUrl,
    ) -> Result<(), StreamsError> {
        // insert_node_addr replaces the whole entry, which would drop the LAN addresses of a peer
        // we already know and leave it undialable while offline.
        let addr = iroh::EndpointAddr::new(p2panda_net::utils::from_verifying_key(node_id))
            .with_relay_url(relay_url);
        if self.inner.node.node_addr_known(&addr).await? {
            return Ok(());
        }
        self.inner.node.insert_node_addr(addr).await?;
        Ok(())
    }

    /// Insert (or refresh) a peer's dialing address in the p2panda address book.
    ///
    /// Always overwrites any existing entry: `insert_node_addr` replaces the
    /// whole `NodeInfo`, resetting its metrics, so an entry marked "stale" by an
    /// earlier failed dial — which `AddressBookDiscovery` then refuses to
    /// resolve, leaving the peer undialable and, since the address book is
    /// persisted, staying that way across restarts — is refreshed and becomes
    /// dialable again. Callers only ever pass mailbox `/health` self-addresses
    /// and addresses a user's opt-in local mailbox forwards, so there is no
    /// untrusted address here to guard an existing entry against.
    //
    // KNOWN LIMITATION: a malicious client that registered this endpoint
    // before p2panda discovered it via mDNS/gossip can continue to inject
    // undialable addresses here (griefing). We cannot detect the upgrade
    // from mailbox-discovered to node-discovered without a
    // p2panda discovery hook;
    // the iroh QUIC handshake prevents data from flowing to the wrong peer,
    // so the worst case is wasted dial attempts.
    pub(crate) async fn register_peer_addr(
        &self,
        addr: iroh::EndpointAddr,
    ) -> Result<(), StreamsError> {
        self.inner.node.insert_node_addr(addr).await?;
        Ok(())
    }

    /// Drop every publisher. Still-parked imports are aborted when the [`Drain`] is dropped.
    pub(crate) async fn shutdown(&self) {
        self.inner.publishers.lock().await.clear();
    }
}

/// Every subscription stream merged into one, pulled by the application processor.
///
/// The application processor must never await a publish into a subscribed topic while it is
/// pulling from here: p2panda's per-topic pipeline is bounded all the way through to the
/// subscription, so such a publish can wait on the very drain it is blocking.
pub(crate) struct Drain {
    node_id: NodeId,

    /// All subscription streams.
    streams: StreamMap<Topic, StreamSubscription<Payload>>,

    processed: Arc<std::sync::Mutex<Processed>>,

    command_rx: mpsc::UnboundedReceiver<DrainCommand>,

    /// Import tasks spawned so an import does not block the drain.
    /// Dropped with the drain, aborting any still-parked imports.
    import_tasks: JoinSet<Option<(Topic, ImportError)>>,
}

impl Drain {
    /// Wait for the next event to process, handling subscription changes in the meantime.
    ///
    /// Returns `None` once [`Streams`] has been dropped. Cancel safe.
    pub(crate) async fn next(&mut self) -> Option<ProcessorEvent> {
        loop {
            select!(
                command = self.command_rx.recv() => {
                    let Some(command) = command else {
                        warn!("streams dropped, exiting drain");
                        return None;
                    };
                    self.handle_command(command);
                }
                Some((_, event)) = self.streams.next() => {
                    if let Some(event) = self.processor_event(event) {
                        return Some(event);
                    }
                }
                Some(result) = self.import_tasks.join_next() => {
                    match result {
                        Ok(Some((topic, error))) => {
                            return Some(ProcessorEvent::ImportFailed { topic, error });
                        }
                        Ok(None) => {}
                        Err(err) => error!(?err, "import task panicked"),
                    }
                }
            );
        }
    }

    fn handle_command(&mut self, command: DrainCommand) {
        match command {
            DrainCommand::Subscribe(topic, rx) => {
                self.streams.insert(topic, rx);
            }
            DrainCommand::Unsubscribe(topic) => {
                self.streams.remove(&topic);
            }
            DrainCommand::Import {
                topic,
                publisher,
                stream,
            } => {
                self.spawn_import(topic, publisher, stream);
            }
        }
    }

    fn spawn_import(
        &mut self,
        topic: Topic,
        publisher: StreamPublisher<Payload>,
        stream: Pin<Box<dyn Stream<Item = Operation> + Send>>,
    ) {
        self.import_tasks.spawn(async move {
            let err = publisher.import(stream).await.err()?;
            error!(topic = ?topic.aliased(), ?err, "import stream failed; topic will not receive further mailbox deliveries until unsubscribed");
            Some((topic, err))
        });
    }

    /// The event to hand to the application processor, logging any system event instead.
    fn processor_event(&mut self, event: StreamEvent<Payload>) -> Option<ProcessorEvent> {
        match event {
            StreamEvent::Processed { operation, source } => {
                let processed_tx = self
                    .processed
                    .lock()
                    .unwrap()
                    .take_sender(operation.id(), operation.author() == self.node_id);
                Some(ProcessorEvent::Operation {
                    operation,
                    source,
                    processed_tx,
                })
            }
            StreamEvent::ProcessingFailed { error, .. } => {
                warn!("error processing operation: {error:?}");
                None
            }
            StreamEvent::DecodeFailed { error, .. } => {
                warn!("error decoding operation: {error:?}");
                None
            }
            StreamEvent::ReplayFailed { error, .. } => {
                warn!("error replaying stream: {error:?}");
                None
            }
            StreamEvent::AckFailed { error, .. } => {
                warn!("error acking operation: {error:?}");
                None
            }
            _ => None,
        }
    }
}

/// Future which can be awaited to find out when a locally published operation has finished
/// system and application layer processing.
pub struct ProcessFuture {
    hash: Hash,
    inner: Pin<Box<dyn Future<Output = <PublishFuture as Future>::Output> + Send + Sync>>,
}

impl ProcessFuture {
    pub fn new(hash: Hash, published_fut: PublishFuture, processed_rx: ProcessedRx) -> Self {
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
pub enum StreamsError {
    #[error(transparent)]
    Publish(#[from] PublishError),

    #[error(transparent)]
    Subscribe(#[from] CreateStreamError),

    #[error(transparent)]
    Import(#[from] ImportError),

    #[error("stream drain is gone")]
    DrainClosed,

    #[error(transparent)]
    Network(#[from] NetworkError),
}

#[derive(Clone, Debug, Error)]
pub enum ProcessorError {
    #[error("application layer processing error: {0}")]
    App(String),
}

#[cfg(test)]
mod tests {
    use futures::future::join_all;
    use p2panda::{Hash, Node, Topic};

    use crate::node::actor::ProcessorEvent;
    use crate::testing::setup_tracing;
    use crate::{ChatMessageContent, ChatPayload, Payload};

    use super::{Processed, Streams};

    fn chat(message: &str) -> Payload {
        Payload::Chat(ChatPayload::Message(ChatMessageContent::text_only(message)))
    }

    #[tokio::test]
    async fn claim_after_forward_still_resolves() {
        let hash = Hash::digest(b"op");
        let mut processed = Processed::default();

        let tx = processed
            .take_sender(hash, true)
            .expect("own op gets a sender");
        tx.send(Ok(())).unwrap();

        let rx = processed.claim(hash);
        assert!(rx.await.unwrap().is_ok());
        assert!(processed.unclaimed.is_empty());
    }

    #[tokio::test]
    async fn claim_before_forward_resolves() {
        let hash = Hash::digest(b"op");
        let mut processed = Processed::default();

        let rx = processed.claim(hash);
        let tx = processed
            .take_sender(hash, false)
            .expect("registered op gets its sender");
        tx.send(Ok(())).unwrap();

        assert!(rx.await.unwrap().is_ok());
        assert!(processed.registered.is_empty());
    }

    #[tokio::test]
    async fn remote_ops_get_no_sender() {
        let mut processed = Processed::default();
        assert!(processed.take_sender(Hash::digest(b"op"), false).is_none());
        assert!(processed.unclaimed.is_empty());
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

        let (alice_streams, alice_drain) = Streams::new(alice, None);
        let (bobbi_streams, bobbi_drain) = Streams::new(bobbi, None);

        // Both alice and bobbi subscribe to topics a & b.
        for topic in [topic_a, topic_b] {
            assert!(alice_streams.subscribe(topic).await.unwrap());
            assert!(bobbi_streams.subscribe(topic).await.unwrap());
        }

        // Alice sends a message into each topic.
        let topic_a_message = chat("hey from topic a!");
        let topic_b_message = chat("hey from topic b!");

        let mut processed_futures = vec![];
        for (topic, payload) in [
            (topic_a, topic_a_message.clone()),
            (topic_b, topic_b_message.clone()),
        ] {
            let processed_future = alice_streams.publish(topic, payload).await.unwrap();
            processed_futures.push(processed_future);
        }

        // Both alice and bobbi receive the messages on their events stream.
        for mut drain in [alice_drain, bobbi_drain] {
            let mut topic_a_message_received = false;
            let mut topic_b_message_received = false;
            while let Some(ProcessorEvent::Operation {
                operation,
                processed_tx,
                ..
            }) = drain.next().await
            {
                if let Some(processed_tx) = processed_tx {
                    let _ = processed_tx.send(Ok(()));
                }

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
}
