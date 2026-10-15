use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures::{Stream, StreamExt};
use p2panda::streams::{EphemeralStreamPublisher, EphemeralStreamSubscription};
use serde::{Deserialize, Serialize};
use tokio::sync::watch;
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

type OnlineDevices = watch::Sender<BTreeSet<DeviceId>>;

/// Presence on chat topics: every node says `Hello` when it joins a topic, sends a heartbeat every
/// [`HEARTBEAT_INTERVAL`] and says `Bye` when it stops. A peer is online on a topic until it says
/// `Bye` or misses its heartbeats for [`OFFLINE_AFTER`].
#[derive(Clone)]
pub struct OnlinePeers {
    me: DeviceId,
    projection: OpProjection,
    online_devices_by_topic: Arc<Mutex<HashMap<TopicId, Arc<OnlineDevices>>>>,
    present_topics: Arc<Mutex<HashMap<TopicId, CancellationToken>>>,
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
            online_devices_by_topic: Default::default(),
            present_topics: Default::default(),
            tasks: TaskTracker::new(),
            _stop_on_drop: Arc::new(stop.clone().drop_guard()),
            stop,
        }
    }

    pub(crate) fn has_joined(&self, topic: TopicId) -> bool {
        self.present_topics.lock().unwrap().contains_key(&topic)
    }

    pub(crate) fn join(&self, topic: TopicId, stream: PresenceStream) {
        let mut present_topics = self.present_topics.lock().unwrap();
        if present_topics.contains_key(&topic) {
            return;
        }
        let stop = self.stop.child_token();
        present_topics.insert(topic, stop.clone());
        self.tasks.spawn(stay_present(
            self.me,
            stream,
            self.projection.clone(),
            TopicPresence::new(self.online_devices(topic)),
            stop,
        ));
    }

    pub(crate) fn leave(&self, topic: TopicId) {
        if let Some(stop) = self.present_topics.lock().unwrap().remove(&topic) {
            stop.cancel();
            self.online_devices(topic).send_if_modified(|devices| {
                let had_peers = !devices.is_empty();
                devices.clear();
                had_peers
            });
        }
    }

    pub(crate) fn forget(&self, device: DeviceId) {
        for online_devices in self.online_devices_by_topic.lock().unwrap().values() {
            online_devices.send_if_modified(|devices| devices.remove(&device));
        }
    }

    pub(crate) async fn leave_all(&self) {
        self.stop.cancel();
        self.tasks.close();
        self.tasks.wait().await;
    }

    pub fn devices(&self, topic: TopicId) -> impl Stream<Item = BTreeSet<DeviceId>> + Send + use<> {
        WatchStream::new(self.online_devices(topic).subscribe())
    }

    fn online_devices(&self, topic: TopicId) -> Arc<OnlineDevices> {
        self.online_devices_by_topic
            .lock()
            .unwrap()
            .entry(topic)
            .or_insert_with(|| Arc::new(watch::channel(BTreeSet::new()).0))
            .clone()
    }
}

/// When each peer of one topic was last heard from, kept by that topic's task, which only
/// publishes the online set when someone joins or leaves.
struct TopicPresence {
    last_seen: HashMap<DeviceId, Instant>,
    online_devices: Arc<OnlineDevices>,
}

impl TopicPresence {
    fn new(online_devices: Arc<OnlineDevices>) -> Self {
        Self {
            last_seen: HashMap::new(),
            online_devices,
        }
    }

    fn seen(&mut self, device: DeviceId) {
        self.last_seen.insert(device, Instant::now());
        self.online_devices
            .send_if_modified(|devices| devices.insert(device));
    }

    fn left(&mut self, device: DeviceId) {
        self.last_seen.remove(&device);
        self.online_devices
            .send_if_modified(|devices| devices.remove(&device));
    }

    fn forget_silent_peers(&mut self) {
        let mut silent = Vec::new();
        self.last_seen.retain(|device, seen| {
            let heard_lately = seen.elapsed() <= OFFLINE_AFTER;
            if !heard_lately {
                silent.push(*device);
            }
            heard_lately
        });
        self.online_devices.send_if_modified(|devices| {
            let before = devices.len();
            for device in &silent {
                devices.remove(device);
            }
            devices.len() != before
        });
    }
}

async fn stay_present(
    me: DeviceId,
    (publisher, mut subscription): PresenceStream,
    projection: OpProjection,
    mut presence: TopicPresence,
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
                return;
            }
            _ = heartbeat.tick() => {
                send(&publisher, PresenceMessage::Heartbeat).await;
                last_sent = Instant::now();
                presence.forget_silent_peers();
            }
            Some(message) = subscription.next() => {
                let device = DeviceId::from(message.author());
                if device == me || matches!(projection.is_author_blocked(&device).await, Ok(true)) {
                    continue;
                }
                match message.body() {
                    PresenceMessage::Hello => {
                        presence.seen(device);
                        // Bring our next heartbeat forward instead of answering, so a burst of joins
                        // lands on one heartbeat that replaces the scheduled one.
                        heartbeat.reset_at((last_sent + MIN_HEARTBEAT_GAP).max(Instant::now()));
                    }
                    PresenceMessage::Heartbeat => presence.seen(device),
                    PresenceMessage::Bye => presence.left(device),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn device(byte: u8) -> DeviceId {
        DeviceId::from(p2panda::SigningKey::from_bytes(&[byte; 32]).verifying_key())
    }

    fn topic_presence() -> TopicPresence {
        TopicPresence::new(Arc::new(watch::channel(BTreeSet::new()).0))
    }

    #[tokio::test(start_paused = true)]
    async fn a_peer_that_stops_sending_heartbeats_goes_offline() {
        let mut presence = topic_presence();
        let (quiet, chatty) = (device(1), device(2));
        presence.seen(quiet);
        presence.seen(chatty);

        tokio::time::advance(OFFLINE_AFTER - HEARTBEAT_INTERVAL).await;
        presence.seen(chatty);
        presence.forget_silent_peers();
        assert_eq!(
            *presence.online_devices.borrow(),
            BTreeSet::from([quiet, chatty])
        );

        tokio::time::advance(HEARTBEAT_INTERVAL + Duration::from_millis(1)).await;
        presence.forget_silent_peers();
        assert_eq!(*presence.online_devices.borrow(), BTreeSet::from([chatty]));
    }

    #[test]
    fn a_peer_that_says_bye_goes_offline_at_once() {
        let mut presence = topic_presence();
        presence.seen(device(1));
        presence.left(device(1));
        assert!(presence.online_devices.borrow().is_empty());
    }

    #[test]
    fn a_heartbeat_from_a_known_peer_wakes_nobody() {
        let mut presence = topic_presence();
        let mut subscriber = presence.online_devices.subscribe();
        presence.seen(device(1));
        subscriber.mark_unchanged();
        presence.seen(device(1));
        assert!(!subscriber.has_changed().unwrap());
    }
}
