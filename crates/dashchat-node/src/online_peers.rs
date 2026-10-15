use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures::{Stream, StreamExt};
use p2panda::streams::{EphemeralStreamPublisher, EphemeralStreamSubscription};
use serde::{Deserialize, Serialize};
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tokio::time::Instant;
use tokio_stream::wrappers::WatchStream;
use tokio_util::sync::{CancellationToken, DropGuard};
use tokio_util::task::TaskTracker;

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

/// When each online device of a topic was last heard from. Subscribers are only woken when a
/// device comes online or goes offline, not when a heartbeat refreshes a time.
type LastSeen = watch::Sender<HashMap<DeviceId, Instant>>;

/// Our presence on one topic: the task that says `Hello`, sends heartbeats and says `Bye`.
struct TopicPresence {
    stop: CancellationToken,
    finished: JoinHandle<()>,
}

impl TopicPresence {
    fn spawn(
        tasks: &TaskTracker,
        stop: CancellationToken,
        me: DeviceId,
        stream: PresenceStream,
        projection: OpProjection,
        last_seen: Arc<LastSeen>,
    ) -> Self {
        let finished = tasks.spawn(stay_present(
            me,
            stream,
            projection,
            last_seen,
            stop.clone(),
        ));
        Self { stop, finished }
    }

    /// Returns once the task has said `Bye` and cleared the topic, so a presence spawned right
    /// after can't have its peers wiped, or its `Hello` overtaken, by this one.
    async fn leave(self) {
        self.stop.cancel();
        let _ = self.finished.await;
    }
}

/// Presence on chat topics: every node says `Hello` when it joins a topic, sends a heartbeat every
/// [`HEARTBEAT_INTERVAL`] and says `Bye` when it stops. A peer is online on a topic until it says
/// `Bye` or misses its heartbeats for [`OFFLINE_AFTER`].
#[derive(Clone)]
pub struct OnlinePeers {
    me: DeviceId,
    projection: OpProjection,
    last_seen_by_topic: Arc<Mutex<HashMap<TopicId, Arc<LastSeen>>>>,
    presence_by_topic: Arc<Mutex<HashMap<TopicId, TopicPresence>>>,
    tasks: TaskTracker,
    stop: CancellationToken,
    _stop_on_drop: Arc<DropGuard>,
}

impl OnlinePeers {
    pub(crate) fn new(me: DeviceId, projection: OpProjection) -> Self {
        let stop = CancellationToken::new();
        Self {
            me,
            projection,
            last_seen_by_topic: Default::default(),
            presence_by_topic: Default::default(),
            tasks: TaskTracker::new(),
            _stop_on_drop: Arc::new(stop.clone().drop_guard()),
            stop,
        }
    }

    pub(crate) fn has_joined(&self, topic: TopicId) -> bool {
        self.presence_by_topic.lock().unwrap().contains_key(&topic)
    }

    pub(crate) fn join(&self, topic: TopicId, stream: PresenceStream) {
        let last_seen = self.last_seen(topic);
        self.presence_by_topic
            .lock()
            .unwrap()
            .entry(topic)
            .or_insert_with(|| {
                TopicPresence::spawn(
                    &self.tasks,
                    self.stop.child_token(),
                    self.me,
                    stream,
                    self.projection.clone(),
                    last_seen,
                )
            });
    }

    pub(crate) async fn leave(&self, topic: TopicId) {
        let presence = self.presence_by_topic.lock().unwrap().remove(&topic);
        if let Some(presence) = presence {
            presence.leave().await;
        }
    }

    pub(crate) fn forget(&self, device: DeviceId) {
        for last_seen in self.last_seen_by_topic.lock().unwrap().values() {
            left(last_seen, device);
        }
    }

    pub(crate) async fn leave_all(&self) {
        self.stop.cancel();
        self.tasks.close();
        self.tasks.wait().await;
    }

    pub fn devices(&self, topic: TopicId) -> impl Stream<Item = BTreeSet<DeviceId>> + Send + use<> {
        WatchStream::new(self.last_seen(topic).subscribe())
            .map(|last_seen| last_seen.keys().copied().collect())
    }

    fn last_seen(&self, topic: TopicId) -> Arc<LastSeen> {
        self.last_seen_by_topic
            .lock()
            .unwrap()
            .entry(topic)
            .or_insert_with(|| Arc::new(watch::channel(HashMap::new()).0))
            .clone()
    }
}

async fn stay_present(
    me: DeviceId,
    (publisher, mut subscription): PresenceStream,
    projection: OpProjection,
    last_seen: Arc<LastSeen>,
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
                // Here rather than in `leave`, so it comes after the last device this task saw.
                forget_everyone(&last_seen);
                return;
            }
            _ = heartbeat.tick() => {
                send(&publisher, PresenceMessage::Heartbeat).await;
                last_sent = Instant::now();
                forget_silent_peers(&last_seen);
            }
            Some(message) = subscription.next() => {
                let device = DeviceId::from(message.author());
                if device == me || matches!(projection.is_author_blocked(&device).await, Ok(true)) {
                    continue;
                }
                match message.body() {
                    PresenceMessage::Hello => {
                        seen(&last_seen, device);
                        // Bring our next heartbeat forward instead of answering, so a burst of joins
                        // lands on one heartbeat that replaces the scheduled one.
                        heartbeat.reset_at((last_sent + MIN_HEARTBEAT_GAP).max(Instant::now()));
                    }
                    PresenceMessage::Heartbeat => seen(&last_seen, device),
                    PresenceMessage::Bye => left(&last_seen, device),
                }
            }
        }
    }
}

fn seen(last_seen: &LastSeen, device: DeviceId) {
    last_seen.send_if_modified(|last_seen| last_seen.insert(device, Instant::now()).is_none());
}

fn left(last_seen: &LastSeen, device: DeviceId) {
    last_seen.send_if_modified(|last_seen| last_seen.remove(&device).is_some());
}

fn forget_silent_peers(last_seen: &LastSeen) {
    last_seen.send_if_modified(|last_seen| {
        let before = last_seen.len();
        last_seen.retain(|_, seen| seen.elapsed() <= OFFLINE_AFTER);
        last_seen.len() != before
    });
}

fn forget_everyone(last_seen: &LastSeen) {
    last_seen.send_if_modified(|last_seen| {
        let had_anyone = !last_seen.is_empty();
        last_seen.clear();
        had_anyone
    });
}

async fn send(publisher: &EphemeralStreamPublisher<PresenceMessage>, message: PresenceMessage) {
    if let Err(err) = publisher.publish(message).await {
        tracing::warn!(?err, ?message, "failed to send presence message");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(byte: u8) -> DeviceId {
        DeviceId::from(p2panda::SigningKey::from_bytes(&[byte; 32]).verifying_key())
    }

    fn online(last_seen: &LastSeen) -> BTreeSet<DeviceId> {
        last_seen.borrow().keys().copied().collect()
    }

    #[tokio::test(start_paused = true)]
    async fn a_peer_that_stops_sending_heartbeats_goes_offline() {
        let last_seen = watch::channel(HashMap::new()).0;
        let (quiet, chatty) = (device(1), device(2));
        seen(&last_seen, quiet);
        seen(&last_seen, chatty);

        tokio::time::advance(OFFLINE_AFTER - HEARTBEAT_INTERVAL).await;
        seen(&last_seen, chatty);
        forget_silent_peers(&last_seen);
        assert_eq!(online(&last_seen), BTreeSet::from([quiet, chatty]));

        tokio::time::advance(HEARTBEAT_INTERVAL + Duration::from_millis(1)).await;
        forget_silent_peers(&last_seen);
        assert_eq!(online(&last_seen), BTreeSet::from([chatty]));
    }

    #[test]
    fn a_peer_that_says_bye_goes_offline_at_once() {
        let last_seen = watch::channel(HashMap::new()).0;
        seen(&last_seen, device(1));
        left(&last_seen, device(1));
        assert!(online(&last_seen).is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn a_heartbeat_from_a_known_peer_wakes_nobody_but_keeps_it_online() {
        let last_seen = watch::channel(HashMap::new()).0;
        let mut subscriber = last_seen.subscribe();
        seen(&last_seen, device(1));
        subscriber.mark_unchanged();

        tokio::time::advance(OFFLINE_AFTER - HEARTBEAT_INTERVAL).await;
        seen(&last_seen, device(1));
        assert!(!subscriber.has_changed().unwrap());

        tokio::time::advance(HEARTBEAT_INTERVAL + Duration::from_millis(1)).await;
        forget_silent_peers(&last_seen);
        assert_eq!(online(&last_seen), BTreeSet::from([device(1)]));
    }
}
