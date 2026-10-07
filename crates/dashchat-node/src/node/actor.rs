use std::collections::{HashMap, VecDeque};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use aliased::Aliasing;
use futures::future::join;
use futures::{FutureExt, Stream};
use p2panda::network::NetworkError;
use p2panda::node::CreateStreamError;
use p2panda::operation::{Extensions, LogId, Operation};
use p2panda::streams::{
    ImportError, ProcessedOperation, PublishError, PublishFuture, Source, StreamEvent, StreamFrom,
    StreamPublisher, StreamSubscription,
};
use p2panda::{Hash, NodeId, RelayUrl, Topic};
use p2panda_auth::group::GroupCrdtError;
use p2panda_stream::Processor;
use p2panda_stream::groups::{GroupsArgs as GroupsProcessorArgs, GroupsError};
use thiserror::Error;
use tokio::select;
use tokio::sync::{mpsc, oneshot};
use tokio::task::{JoinHandle, JoinSet};
use tokio_stream::{StreamExt, StreamMap};
use tracing::{error, warn};

use crate::Payload;

type GroupsProcessor = p2panda_stream::groups::Groups<GroupsProcessorArgs<()>, Extensions, LogId>;

type ProcessedTx = oneshot::Sender<Result<(), ProcessorError>>;
type ProcessedRx = oneshot::Receiver<Result<(), ProcessorError>>;

/// Own-authored operations the drain task forwarded before `publish` claimed them (see
/// [`Processed`]). The window is the gap between p2panda returning the hash and the claim, so
/// this only ever holds a handful; the cap just guards against operations we authored through
/// other routes (replays, local imports) and never claim.
const UNCLAIMED_CAPACITY: usize = 1024;

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
// @TODO: This wrapping might not have been required if the stream draining and app processing
// pipeline was combined into one process. I(sam) avoided doing that so as to keep my work as self
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
        processed_tx: Option<ProcessedTx>,
        error: Option<ProcessorError>,
    },
    App {
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
/// This is a thin wrapper around the p2panda node API which holds all publish handles and runs
/// one task draining every subscription stream. That task also processes groups control messages
/// as they arrive and lets callers await that processing for operations they published locally.
///
/// Publishing and subscribing are plain methods: the only state confined to the drain task is the
/// merged subscription streams and the groups processor, so calls never queue behind event
/// processing the way an actor command would.
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
    drain_handle: std::sync::Mutex<Option<JoinHandle<()>>>,
}

/// One shot channels for locally published operations which resolve once the operation has
/// completed additional processing.
///
/// These are held while groups control messages are being processed so the user can await this
/// processing on top of what the node already provides with PublishFuture. The oneshot channel
/// sender is forwarded further up the processing pipeline (to the application layer) so that any
/// further processing which occurs there can also be awaited.
///
/// p2panda hands an operation to the pipeline before `publish` learns its hash, so the drain task
/// can forward it before the publisher registers interest. For own-authored operations the drain
/// task then attaches a sender of its own and parks the receiver in `unclaimed` for `publish` to
/// pick up.
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
    pub(crate) fn new(
        node: p2panda::Node,
        stream_cursor_prefix: Option<String>,
    ) -> (Self, mpsc::UnboundedReceiver<ProcessorEvent>) {
        // Unbounded so the drain task never blocks here: the application processor
        // (the only consumer) itself publishes and awaits the result, so a bounded
        // channel deadlocks under a burst of events (see
        // `late_joiner_syncing_crossing_replies_can_hit_target_not_found` in
        // tests/reply_messages.rs, which used to hang this way).
        let (events_tx, events_rx) = mpsc::unbounded_channel();
        let (drain_tx, drain_rx) = mpsc::unbounded_channel();
        let processed = Arc::new(std::sync::Mutex::new(Processed::default()));

        let drain = Drain {
            node_id: node.id(),
            streams: Default::default(),
            groups_processor: GroupsProcessor::new(node.store()),
            processed: processed.clone(),
            events_tx,
            import_tasks: JoinSet::new(),
        };
        let drain_handle = tokio::spawn(drain.run(drain_rx));

        let streams = Self {
            inner: Arc::new(Inner {
                node,
                stream_cursor_prefix,
                publishers: Default::default(),
                processed,
                drain_tx,
                drain_handle: std::sync::Mutex::new(Some(drain_handle)),
            }),
        };

        (streams, events_rx)
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
        let publish_fut = match &payload {
            Payload::GroupControl(args) => tx.publish_groups(args.clone(), payload).await,
            _ => tx.publish(payload).await,
        }?;

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

    /// Drop every publisher and stop the drain task, aborting any still-parked imports.
    pub(crate) async fn shutdown(&self) {
        self.inner.publishers.lock().await.clear();
        let handle = self.inner.drain_handle.lock().unwrap().take();
        if let Some(handle) = handle {
            handle.abort();
            let _ = handle.await;
        }
    }
}

/// State confined to the task draining all subscription streams.
struct Drain {
    node_id: NodeId,

    /// All subscription streams.
    streams: StreamMap<Topic, StreamSubscription<Payload>>,

    /// Groups processor.
    groups_processor: GroupsProcessor,

    processed: Arc<std::sync::Mutex<Processed>>,

    /// Channel for forwarding all received events on to the application layer processor.
    events_tx: mpsc::UnboundedSender<ProcessorEvent>,

    /// Import tasks spawned so an import does not block the drain loop.
    /// Dropped on shutdown, aborting any still-parked imports.
    import_tasks: JoinSet<()>,
}

impl Drain {
    async fn run(mut self, mut command_rx: mpsc::UnboundedReceiver<DrainCommand>) {
        loop {
            select!(
                command = command_rx.recv() => {
                    match command {
                        Some(DrainCommand::Subscribe(topic, rx)) => {
                            self.streams.insert(topic, rx);
                        }
                        Some(DrainCommand::Unsubscribe(topic)) => {
                            self.streams.remove(&topic);
                        }
                        Some(DrainCommand::Import { topic, publisher, stream }) => {
                            self.spawn_import(topic, publisher, stream);
                        }
                        None => {
                            warn!("streams dropped, exiting drain loop");
                            break;
                        }
                    }
                }
                Some((_, event)) = self.streams.next() => {
                    if let Err(err) = self.process_event(event).await {
                        warn!(?err, "stream event processing failed");
                    }
                }
                Some(result) = self.import_tasks.join_next() => {
                    if let Err(err) = result {
                        error!(?err, "import task panicked");
                    }
                }
            );
        }
    }

    fn spawn_import(
        &mut self,
        topic: Topic,
        publisher: StreamPublisher<Payload>,
        stream: Pin<Box<dyn Stream<Item = Operation> + Send>>,
    ) {
        let events_tx = self.events_tx.clone();
        self.import_tasks.spawn(async move {
            if let Err(err) = publisher.import(stream).await {
                error!(topic = ?topic.aliased(), ?err, "import stream failed; topic will not receive further mailbox deliveries until unsubscribed");
                let _ = events_tx.send(ProcessorEvent::ImportFailed { topic, error: err });
            }
        });
    }

    async fn process_event(&mut self, event: StreamEvent<Payload>) -> Result<(), StreamsError> {
        let processor_event = match &event {
            StreamEvent::Processed { operation, source } => {
                let processed_tx = self
                    .processed
                    .lock()
                    .unwrap()
                    .take_sender(operation.id(), operation.author() == self.node_id);

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
            .map_err(|_| StreamsError::EventSend)?;

        Ok(())
    }

    async fn process_groups_control(
        &self,
        operation: &ProcessedOperation<Payload>,
    ) -> Result<(), ProcessorError> {
        match self
            .groups_processor
            .process(operation.event.groups_args.clone())
            .await
        {
            Ok(()) => {}
            // Another process sharing the groups state (the iOS push extension)
            // already applied it. A failed `process` enqueues nothing, so there
            // is nothing to drain.
            Err((_, GroupsError::Groups(GroupCrdtError::DuplicateOperation(..)))) => {
                return Ok(());
            }
            Err((_, err)) => return Err(ProcessorError::Groups(err.to_string())),
        }

        // A successful `process` always enqueues exactly one item, no-ops included, and this is
        // the only caller (the single drain loop), so `next` returns our own item without
        // blocking. It carries only the input back plus a processed/no-op flag, so dropping it
        // loses nothing; we drain it so the queue doesn't grow unboundedly.
        //
        // Note that this is only a temporary solution anyway and will go away with the spaces refactor.
        self.groups_processor
            .next()
            .await
            .map(|_| ())
            .map_err(|(_, err)| ProcessorError::Groups(err.to_string()))
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

    #[error("error sending on event tx")]
    EventSend,

    #[error("stream drain task is gone")]
    DrainClosed,

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
    use p2panda::groups::GroupsArgs;
    use p2panda::{Hash, Node, SigningKey, Topic, VerifyingKey};
    use p2panda_auth::Access;
    use p2panda_auth::group::{GroupAction, GroupCrdtState, GroupMember};
    use p2panda_store::groups::GroupsStore;
    use p2panda_store::{SqliteStore, tx_unwrap};
    use p2panda_stream::groups::GroupsOperation;

    use crate::node::actor::ProcessorEvent;
    use crate::stores::GROUPS_STATE_ID;
    use crate::testing::setup_tracing;
    use crate::{ChatMessageContent, ChatPayload, Payload};

    use super::{Processed, Streams};

    type GroupsState = GroupCrdtState<VerifyingKey, Hash, GroupsOperation, ()>;

    fn chat(message: &str) -> Payload {
        Payload::Chat(ChatPayload::Message(ChatMessageContent::text_only(message)))
    }

    async fn groups_control(
        store: &SqliteStore,
        group_id: VerifyingKey,
        action: GroupAction<VerifyingKey>,
    ) -> Payload {
        let groups_y: GroupsState =
            tx_unwrap!(store, { store.get_groups_state_tx(*GROUPS_STATE_ID).await })
                .unwrap()
                .unwrap_or_default();

        let dependencies = groups_y.heads(&[group_id]);
        Payload::GroupControl(GroupsArgs {
            group_id,
            action,
            dependencies,
        })
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

        let (alice_streams, alice_events_rx) = Streams::new(alice, None);
        let (bobbi_streams, bobbi_events_rx) = Streams::new(bobbi, None);

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
        for mut events_rx in [alice_events_rx, bobbi_events_rx] {
            let mut topic_a_message_received = false;
            let mut topic_b_message_received = false;
            while let Some(ProcessorEvent::App {
                operation,
                processed_tx,
                ..
            }) = events_rx.recv().await
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

    #[tokio::test(flavor = "multi_thread")]
    async fn process_groups_control_messages() {
        setup_tracing(&["dashchat=info", "aliased=warn"], true);

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

        let (alice_streams, alice_events_rx) = Streams::new(alice, None);
        let (bobbi_streams, bobbi_events_rx) = Streams::new(bobbi, None);

        assert!(alice_streams.subscribe(topic).await.unwrap());
        assert!(bobbi_streams.subscribe(topic).await.unwrap());

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

        let processed_fut = alice_streams
            .publish(topic, create_group.clone())
            .await
            .unwrap();

        // Both receive the message on their events stream.
        for mut events_rx in [alice_events_rx, bobbi_events_rx] {
            while let Some(event) = events_rx.recv().await {
                if let ProcessorEvent::Groups {
                    operation,
                    processed_tx,
                    ..
                } = event
                {
                    if let Some(processed_tx) = processed_tx {
                        let _ = processed_tx.send(Ok(()));
                    }
                    if operation.message() == &create_group {
                        break;
                    }
                }
            }
        }

        assert!(processed_fut.await.unwrap().is_completed());

        // And they have also processed the groups control message.
        for store in [alice_store, bobbi_store] {
            let groups_y: GroupsState =
                tx_unwrap!(store, { store.get_groups_state_tx(*GROUPS_STATE_ID).await })
                    .unwrap()
                    .unwrap();
            let members = groups_y.members(group_id);
            assert!(members.contains(&(alice_id, Access::manage())));
            assert!(members.contains(&(bobbi_id, Access::manage())));
        }
    }
}
