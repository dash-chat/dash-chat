use std::collections::{BTreeMap, BTreeSet, HashMap};
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

type LastSeen = BTreeMap<TopicId, BTreeMap<DeviceId, Instant>>;

/// Presence on chat topics: every node says `Hello` when it joins a topic, sends a heartbeat every
/// [`HEARTBEAT_INTERVAL`] and says `Bye` when it stops. A peer is online on a topic until it says
/// `Bye` or misses its heartbeats for [`OFFLINE_AFTER`].
#[derive(Clone)]
pub struct OnlinePeers {
    me: DeviceId,
    projection: OpProjection,
    last_seen: Arc<watch::Sender<LastSeen>>,
    joined: Arc<Mutex<HashMap<TopicId, CancellationToken>>>,
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
            last_seen: Arc::new(watch::channel(LastSeen::new()).0),
            joined: Default::default(),
            tasks: TaskTracker::new(),
            _stop_on_drop: Arc::new(stop.clone().drop_guard()),
            stop,
        }
    }

    pub(crate) fn has_joined(&self, topic: TopicId) -> bool {
        self.joined.lock().unwrap().contains_key(&topic)
    }

    pub(crate) fn join(&self, topic: TopicId, stream: PresenceStream) {
        let mut joined = self.joined.lock().unwrap();
        if joined.contains_key(&topic) {
            return;
        }
        let stop = self.stop.child_token();
        joined.insert(topic, stop.clone());
        self.tasks.spawn(stay_present(
            self.me,
            topic,
            stream,
            self.projection.clone(),
            self.last_seen.clone(),
            stop,
        ));
    }

    pub(crate) fn leave(&self, topic: TopicId) {
        if let Some(stop) = self.joined.lock().unwrap().remove(&topic) {
            stop.cancel();
            self.last_seen
                .send_if_modified(|last_seen| last_seen.remove(&topic).is_some());
        }
    }

    pub(crate) fn forget(&self, device: DeviceId) {
        self.last_seen.send_if_modified(|last_seen| {
            let mut forgot = false;
            for devices in last_seen.values_mut() {
                if devices.remove(&device).is_some() {
                    forgot = true;
                }
            }
            forgot
        });
    }

    pub(crate) async fn leave_all(&self) {
        self.stop.cancel();
        self.tasks.close();
        self.tasks.wait().await;
    }

    pub fn devices(&self, topic: TopicId) -> impl Stream<Item = BTreeSet<DeviceId>> + Send + use<> {
        WatchStream::new(self.last_seen.subscribe()).map(move |last_seen| {
            last_seen
                .get(&topic)
                .map(|devices| devices.keys().copied().collect())
                .unwrap_or_default()
        })
    }
}

async fn stay_present(
    me: DeviceId,
    topic: TopicId,
    (publisher, mut subscription): PresenceStream,
    projection: OpProjection,
    last_seen: Arc<watch::Sender<LastSeen>>,
    stop: CancellationToken,
) {
    send(&publisher, PresenceMessage::Hello).await;
    let mut heartbeat =
        tokio::time::interval_at(Instant::now() + HEARTBEAT_INTERVAL, HEARTBEAT_INTERVAL);
    loop {
        tokio::select! {
            _ = stop.cancelled() => {
                send(&publisher, PresenceMessage::Bye).await;
                return;
            }
            _ = heartbeat.tick() => {
                send(&publisher, PresenceMessage::Heartbeat).await;
                forget_silent_peers(&last_seen, topic);
            }
            Some(message) = subscription.next() => {
                let device = DeviceId::from(message.author());
                if device == me || matches!(projection.is_author_blocked(&device).await, Ok(true)) {
                    continue;
                }
                match message.body() {
                    PresenceMessage::Hello => {
                        seen(&last_seen, topic, device);
                        // Answer right away so the newcomer doesn't wait a whole interval to see us.
                        send(&publisher, PresenceMessage::Heartbeat).await;
                    }
                    PresenceMessage::Heartbeat => seen(&last_seen, topic, device),
                    PresenceMessage::Bye => left(&last_seen, topic, device),
                }
            }
        }
    }
}

fn seen(last_seen: &watch::Sender<LastSeen>, topic: TopicId, device: DeviceId) {
    last_seen.send_if_modified(|last_seen| {
        last_seen
            .entry(topic)
            .or_default()
            .insert(device, Instant::now())
            .is_none()
    });
}

fn left(last_seen: &watch::Sender<LastSeen>, topic: TopicId, device: DeviceId) {
    last_seen.send_if_modified(|last_seen| {
        last_seen
            .get_mut(&topic)
            .is_some_and(|devices| devices.remove(&device).is_some())
    });
}

fn forget_silent_peers(last_seen: &watch::Sender<LastSeen>, topic: TopicId) {
    last_seen.send_if_modified(|last_seen| {
        let Some(devices) = last_seen.get_mut(&topic) else {
            return false;
        };
        let before = devices.len();
        devices.retain(|_, seen| seen.elapsed() <= OFFLINE_AFTER);
        devices.len() != before
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

    fn online(last_seen: &watch::Sender<LastSeen>, topic: TopicId) -> BTreeSet<DeviceId> {
        last_seen
            .borrow()
            .get(&topic)
            .map(|devices| devices.keys().copied().collect())
            .unwrap_or_default()
    }

    #[tokio::test(start_paused = true)]
    async fn a_peer_that_stops_sending_heartbeats_goes_offline() {
        let last_seen = watch::channel(LastSeen::new()).0;
        let topic = TopicId::random();
        let (quiet, chatty) = (device(1), device(2));
        seen(&last_seen, topic, quiet);
        seen(&last_seen, topic, chatty);

        tokio::time::advance(OFFLINE_AFTER - HEARTBEAT_INTERVAL).await;
        seen(&last_seen, topic, chatty);
        forget_silent_peers(&last_seen, topic);
        assert_eq!(online(&last_seen, topic), BTreeSet::from([quiet, chatty]));

        tokio::time::advance(HEARTBEAT_INTERVAL + Duration::from_millis(1)).await;
        forget_silent_peers(&last_seen, topic);
        assert_eq!(online(&last_seen, topic), BTreeSet::from([chatty]));
    }

    #[test]
    fn a_peer_that_says_bye_goes_offline_at_once() {
        let last_seen = watch::channel(LastSeen::new()).0;
        let topic = TopicId::random();
        seen(&last_seen, topic, device(1));
        left(&last_seen, topic, device(1));
        assert!(online(&last_seen, topic).is_empty());
    }
}
