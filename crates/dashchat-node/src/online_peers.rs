use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures::{Stream, StreamExt};
use p2panda::streams::{EphemeralStreamPublisher, EphemeralStreamSubscription};
use serde::{Deserialize, Serialize};
use tokio::sync::watch;
use tokio::time::Instant;
use tokio_stream::wrappers::WatchStream;
use tokio_util::sync::CancellationToken;
use tokio_util::task::AbortOnDropHandle;

use crate::stores::OpProjection;
use crate::{DeviceId, TopicId};

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
const OFFLINE_AFTER: Duration = Duration::from_secs(12);
const MIN_HEARTBEAT_GAP: Duration = Duration::from_secs(1);

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub(crate) enum PresenceMessage {
    Hello,
    Heartbeat,
    Bye,
}

pub(crate) type PresenceStream = (
    EphemeralStreamPublisher<PresenceMessage>,
    EphemeralStreamSubscription<PresenceMessage>,
);

/// Presence on chat topics: every node says `Hello` when it joins a topic, sends a heartbeat every
/// [`HEARTBEAT_INTERVAL`] and says `Bye` when it stops. A peer is online on a topic until it says
/// `Bye` or misses its heartbeats for [`OFFLINE_AFTER`].
#[derive(Clone)]
pub struct OnlinePeers {
    me: DeviceId,
    projection: OpProjection,
    presence_by_topic: Arc<Mutex<HashMap<TopicId, TopicPresence>>>,
}

struct TopicPresence {
    peers: Arc<TopicPeers>,
    /// `Some` while we are present on the topic.
    task: Option<PresenceTask>,
}

impl TopicPresence {
    fn new() -> Self {
        Self {
            peers: Arc::new(TopicPeers::new()),
            task: None,
        }
    }
}

impl OnlinePeers {
    pub(crate) fn new(me: DeviceId, projection: OpProjection) -> Self {
        Self {
            me,
            projection,
            presence_by_topic: Default::default(),
        }
    }

    pub(crate) fn has_joined(&self, topic: TopicId) -> bool {
        self.presence_by_topic
            .lock()
            .unwrap()
            .get(&topic)
            .is_some_and(|presence| presence.task.is_some())
    }

    pub(crate) fn join(&self, topic: TopicId, stream: PresenceStream) {
        let mut presence_by_topic = self.presence_by_topic.lock().unwrap();
        let presence = presence_by_topic
            .entry(topic)
            .or_insert_with(TopicPresence::new);
        if presence.task.is_none() {
            presence.task = Some(PresenceTask::spawn(
                self.me,
                stream,
                self.projection.clone(),
                presence.peers.clone(),
            ));
        }
    }

    pub(crate) async fn leave(&self, topic: TopicId) {
        let task = self
            .presence_by_topic
            .lock()
            .unwrap()
            .get_mut(&topic)
            .and_then(|presence| presence.task.take());
        if let Some(task) = task {
            task.leave().await;
        }
    }

    pub(crate) fn forget(&self, device: DeviceId) {
        for presence in self.presence_by_topic.lock().unwrap().values() {
            presence.peers.forget(device);
        }
    }

    pub(crate) async fn leave_all(&self) {
        let tasks: Vec<_> = self
            .presence_by_topic
            .lock()
            .unwrap()
            .values_mut()
            .filter_map(|presence| presence.task.take())
            .collect();
        futures::future::join_all(tasks.into_iter().map(PresenceTask::leave)).await;
    }

    pub fn devices(&self, topic: TopicId) -> impl Stream<Item = BTreeSet<DeviceId>> + Send + use<> {
        self.presence_by_topic
            .lock()
            .unwrap()
            .entry(topic)
            .or_insert_with(TopicPresence::new)
            .peers
            .online()
    }
}

/// Our presence on one topic: says `Hello`, sends heartbeats and says `Bye` when it leaves.
struct PresenceTask {
    stop: CancellationToken,
    finished: AbortOnDropHandle<()>,
}

impl PresenceTask {
    fn spawn(
        me: DeviceId,
        stream: PresenceStream,
        projection: OpProjection,
        peers: Arc<TopicPeers>,
    ) -> Self {
        let stop = CancellationToken::new();
        let finished = AbortOnDropHandle::new(tokio::spawn(stay_present(
            me,
            stream,
            projection,
            peers,
            stop.clone(),
        )));
        Self { stop, finished }
    }

    /// Returns once we have said `Bye` and forgotten the topic's peers, so a task spawned right
    /// after can't have its peers wiped, or its `Hello` overtaken, by this one.
    async fn leave(self) {
        self.stop.cancel();
        let _ = self.finished.await;
    }
}

async fn stay_present(
    me: DeviceId,
    (publisher, mut subscription): PresenceStream,
    projection: OpProjection,
    peers: Arc<TopicPeers>,
    stop: CancellationToken,
) {
    send(&publisher, PresenceMessage::Hello).await;
    let mut last_sent = Instant::now();
    let mut heartbeat =
        tokio::time::interval_at(last_sent + HEARTBEAT_INTERVAL, HEARTBEAT_INTERVAL);
    loop {
        tokio::select! {
            _ = stop.cancelled() => {
                send(&publisher, PresenceMessage::Bye).await;
                peers.forget_everyone();
                return;
            }
            _ = heartbeat.tick() => {
                send(&publisher, PresenceMessage::Heartbeat).await;
                last_sent = Instant::now();
                peers.forget_silent();
            }
            Some(message) = subscription.next() => {
                let device = DeviceId::from(message.author());
                if device == me || matches!(projection.is_author_blocked(&device).await, Ok(true)) {
                    continue;
                }
                match message.body() {
                    PresenceMessage::Hello => {
                        peers.heard_from(device);
                        // Bring our next heartbeat forward instead of answering, so a burst of joins
                        // lands on one heartbeat that replaces the scheduled one.
                        heartbeat.reset_at((last_sent + MIN_HEARTBEAT_GAP).max(Instant::now()));
                    }
                    PresenceMessage::Heartbeat => peers.heard_from(device),
                    PresenceMessage::Bye => peers.forget(device),
                }
            }
        }
    }
}

async fn send(publisher: &EphemeralStreamPublisher<PresenceMessage>, message: PresenceMessage) {
    if let Err(err) = publisher.publish(message).await {
        tracing::warn!(?err, ?message, "failed to send presence message");
    }
}

/// Who we hear from on one topic, and when we last did. Subscribers are only woken when someone
/// comes online or goes offline, not when a heartbeat refreshes a time.
struct TopicPeers(watch::Sender<HashMap<DeviceId, Instant>>);

impl TopicPeers {
    fn new() -> Self {
        Self(watch::channel(HashMap::new()).0)
    }

    fn heard_from(&self, device: DeviceId) {
        self.0
            .send_if_modified(|last_seen| last_seen.insert(device, Instant::now()).is_none());
    }

    fn forget(&self, device: DeviceId) {
        self.0
            .send_if_modified(|last_seen| last_seen.remove(&device).is_some());
    }

    fn forget_silent(&self) {
        self.0.send_if_modified(|last_seen| {
            let before = last_seen.len();
            last_seen.retain(|_, seen| seen.elapsed() <= OFFLINE_AFTER);
            last_seen.len() != before
        });
    }

    fn forget_everyone(&self) {
        self.0.send_if_modified(|last_seen| {
            let had_anyone = !last_seen.is_empty();
            last_seen.clear();
            had_anyone
        });
    }

    fn online(&self) -> impl Stream<Item = BTreeSet<DeviceId>> + Send + use<> {
        WatchStream::new(self.0.subscribe()).map(|last_seen| last_seen.keys().copied().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(byte: u8) -> DeviceId {
        DeviceId::from(p2panda::SigningKey::from_bytes(&[byte; 32]).verifying_key())
    }

    fn online(peers: &TopicPeers) -> BTreeSet<DeviceId> {
        peers.0.borrow().keys().copied().collect()
    }

    #[tokio::test(start_paused = true)]
    async fn a_peer_that_stops_sending_heartbeats_goes_offline() {
        let peers = TopicPeers::new();
        let (quiet, chatty) = (device(1), device(2));
        peers.heard_from(quiet);
        peers.heard_from(chatty);

        tokio::time::advance(OFFLINE_AFTER - HEARTBEAT_INTERVAL).await;
        peers.heard_from(chatty);
        peers.forget_silent();
        assert_eq!(online(&peers), BTreeSet::from([quiet, chatty]));

        tokio::time::advance(HEARTBEAT_INTERVAL + Duration::from_millis(1)).await;
        peers.forget_silent();
        assert_eq!(online(&peers), BTreeSet::from([chatty]));
    }

    #[test]
    fn a_forgotten_peer_goes_offline_at_once() {
        let peers = TopicPeers::new();
        peers.heard_from(device(1));
        peers.forget(device(1));
        assert!(online(&peers).is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn a_heartbeat_from_a_known_peer_wakes_nobody_but_keeps_it_online() {
        let peers = TopicPeers::new();
        let mut subscriber = peers.0.subscribe();
        peers.heard_from(device(1));
        subscriber.mark_unchanged();

        tokio::time::advance(OFFLINE_AFTER - HEARTBEAT_INTERVAL).await;
        peers.heard_from(device(1));
        assert!(!subscriber.has_changed().unwrap());

        tokio::time::advance(HEARTBEAT_INTERVAL + Duration::from_millis(1)).await;
        peers.forget_silent();
        assert_eq!(online(&peers), BTreeSet::from([device(1)]));
    }
}
