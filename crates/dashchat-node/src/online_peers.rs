use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;
use std::time::Duration;

use futures::{Stream, StreamExt};
use p2panda::streams::{EphemeralStreamPublisher, EphemeralStreamSubscription};
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, watch};
use tokio::time::Instant;
use tokio_stream::wrappers::WatchStream;
use tokio_util::task::AbortOnDropHandle;

use crate::{DeviceId, TopicId};

const PING_INTERVAL: Duration = Duration::from_secs(5);
const OFFLINE_AFTER: Duration = Duration::from_secs(12);

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub(crate) enum PresenceMessage {
    Hello,
    Ping,
    Bye,
}

type LastSeenByTopic = BTreeMap<TopicId, BTreeMap<DeviceId, Instant>>;

type Publishers = HashMap<TopicId, EphemeralStreamPublisher<PresenceMessage>>;

/// Presence on chat topics: every node says `Hello` when it joins a topic, pings it every
/// [`PING_INTERVAL`] and says `Bye` when it stops. A peer is online on a topic until it says
/// `Bye` or misses its pings for [`OFFLINE_AFTER`].
#[derive(Clone)]
pub struct OnlinePeers {
    me: DeviceId,
    last_seen: Arc<watch::Sender<LastSeenByTopic>>,
    publishers: Arc<Mutex<Publishers>>,
    tasks: Arc<std::sync::Mutex<Vec<AbortOnDropHandle<()>>>>,
}

impl OnlinePeers {
    pub(crate) fn new(me: DeviceId) -> Self {
        let online_peers = Self {
            me,
            last_seen: Arc::new(watch::channel(LastSeenByTopic::new()).0),
            publishers: Default::default(),
            tasks: Default::default(),
        };
        let pinging = tokio::spawn(ping_and_expire(
            online_peers.publishers.clone(),
            online_peers.last_seen.clone(),
        ));
        online_peers.keep(pinging);
        online_peers
    }

    pub(crate) async fn has_joined(&self, topic: TopicId) -> bool {
        self.publishers.lock().await.contains_key(&topic)
    }

    pub(crate) async fn join(
        &self,
        topic: TopicId,
        (publisher, subscription): (
            EphemeralStreamPublisher<PresenceMessage>,
            EphemeralStreamSubscription<PresenceMessage>,
        ),
    ) {
        let receiving = tokio::spawn(receive(
            self.me,
            topic,
            subscription,
            self.publishers.clone(),
            self.last_seen.clone(),
        ));
        self.keep(receiving);
        send(&publisher, PresenceMessage::Hello).await;
        self.publishers.lock().await.insert(topic, publisher);
    }

    pub(crate) async fn say_bye_and_stop(&self) {
        for publisher in self
            .publishers
            .lock()
            .await
            .drain()
            .map(|(_, publisher)| publisher)
        {
            send(&publisher, PresenceMessage::Bye).await;
        }
        self.tasks.lock().unwrap().clear();
    }

    pub fn devices(&self, topic: TopicId) -> impl Stream<Item = BTreeSet<DeviceId>> + Send + use<> {
        WatchStream::new(self.last_seen.subscribe()).map(move |last_seen| {
            last_seen
                .get(&topic)
                .map(|devices| devices.keys().copied().collect())
                .unwrap_or_default()
        })
    }

    fn keep(&self, task: tokio::task::JoinHandle<()>) {
        self.tasks
            .lock()
            .unwrap()
            .push(AbortOnDropHandle::new(task));
    }
}

async fn ping_and_expire(
    publishers: Arc<Mutex<Publishers>>,
    last_seen: Arc<watch::Sender<LastSeenByTopic>>,
) {
    let mut interval = tokio::time::interval(PING_INTERVAL);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        interval.tick().await;
        let publishers: Vec<_> = publishers.lock().await.values().cloned().collect();
        for publisher in publishers {
            send(&publisher, PresenceMessage::Ping).await;
        }
        forget_silent(&last_seen);
    }
}

fn forget_silent(last_seen: &watch::Sender<LastSeenByTopic>) {
    last_seen.send_if_modified(|last_seen| {
        let mut forgot = false;
        for devices in last_seen.values_mut() {
            let before = devices.len();
            devices.retain(|_, seen| seen.elapsed() <= OFFLINE_AFTER);
            forgot |= devices.len() != before;
        }
        last_seen.retain(|_, devices| !devices.is_empty());
        forgot
    });
}

async fn receive(
    me: DeviceId,
    topic: TopicId,
    mut subscription: EphemeralStreamSubscription<PresenceMessage>,
    publishers: Arc<Mutex<Publishers>>,
    last_seen: Arc<watch::Sender<LastSeenByTopic>>,
) {
    while let Some(message) = subscription.next().await {
        let device = DeviceId::from(message.author());
        if device == me {
            continue;
        }
        match message.body() {
            PresenceMessage::Hello => {
                seen(&last_seen, topic, device);
                // Answer right away so the newcomer doesn't wait a whole interval to see us.
                let publisher = publishers.lock().await.get(&topic).cloned();
                if let Some(publisher) = publisher {
                    send(&publisher, PresenceMessage::Ping).await;
                }
            }
            PresenceMessage::Ping => seen(&last_seen, topic, device),
            PresenceMessage::Bye => left(&last_seen, topic, device),
        }
    }
}

fn seen(last_seen: &watch::Sender<LastSeenByTopic>, topic: TopicId, device: DeviceId) {
    last_seen.send_if_modified(|last_seen| {
        last_seen
            .entry(topic)
            .or_default()
            .insert(device, Instant::now())
            .is_none()
    });
}

fn left(last_seen: &watch::Sender<LastSeenByTopic>, topic: TopicId, device: DeviceId) {
    last_seen.send_if_modified(|last_seen| {
        let Some(devices) = last_seen.get_mut(&topic) else {
            return false;
        };
        let removed = devices.remove(&device).is_some();
        if devices.is_empty() {
            last_seen.remove(&topic);
        }
        removed
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

    fn online(last_seen: &watch::Sender<LastSeenByTopic>, topic: TopicId) -> BTreeSet<DeviceId> {
        last_seen
            .borrow()
            .get(&topic)
            .map(|devices| devices.keys().copied().collect())
            .unwrap_or_default()
    }

    #[tokio::test(start_paused = true)]
    async fn a_peer_that_stops_pinging_goes_offline() {
        let last_seen = watch::channel(LastSeenByTopic::new()).0;
        let topic = TopicId::random();
        let (quiet, chatty) = (device(1), device(2));
        seen(&last_seen, topic, quiet);
        seen(&last_seen, topic, chatty);

        tokio::time::advance(OFFLINE_AFTER - PING_INTERVAL).await;
        seen(&last_seen, topic, chatty);
        forget_silent(&last_seen);
        assert_eq!(online(&last_seen, topic), BTreeSet::from([quiet, chatty]));

        tokio::time::advance(PING_INTERVAL + Duration::from_millis(1)).await;
        forget_silent(&last_seen);
        assert_eq!(online(&last_seen, topic), BTreeSet::from([chatty]));
    }

    #[test]
    fn a_peer_that_says_bye_goes_offline_at_once() {
        let last_seen = watch::channel(LastSeenByTopic::new()).0;
        let topic = TopicId::random();
        seen(&last_seen, topic, device(1));
        left(&last_seen, topic, device(1));
        assert!(online(&last_seen, topic).is_empty());
    }
}
