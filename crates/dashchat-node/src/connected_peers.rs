use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use futures::{Stream, StreamExt};
use tokio::sync::watch;
use tokio_stream::wrappers::WatchStream;

use crate::{DeviceId, TopicId};

type SessionId = u64;

type SessionsByTopic = BTreeMap<TopicId, BTreeMap<SessionId, DeviceId>>;

#[derive(Clone)]
pub struct ConnectedPeers(Arc<watch::Sender<SessionsByTopic>>);

impl Default for ConnectedPeers {
    fn default() -> Self {
        Self(Arc::new(watch::channel(SessionsByTopic::new()).0))
    }
}

impl ConnectedPeers {
    pub(crate) fn session_opened(&self, topic: TopicId, session_id: SessionId, device: DeviceId) {
        tracing::debug!(?topic, session_id, ?device, "p2p session opened");
        self.0.send_modify(|sessions| {
            sessions
                .entry(topic)
                .or_default()
                .insert(session_id, device);
        });
    }

    pub(crate) fn session_closed(&self, topic: TopicId, session_id: SessionId) {
        tracing::debug!(?topic, session_id, "p2p session closed");
        self.0.send_if_modified(|sessions| {
            let Some(topic_sessions) = sessions.get_mut(&topic) else {
                return false;
            };
            let removed = topic_sessions.remove(&session_id).is_some();
            if topic_sessions.is_empty() {
                sessions.remove(&topic);
            }
            removed
        });
    }

    pub fn connected_devices(
        &self,
        topic: TopicId,
    ) -> impl Stream<Item = BTreeSet<DeviceId>> + Send + use<> {
        WatchStream::new(self.0.subscribe()).map(move |sessions| {
            sessions
                .get(&topic)
                .map(|topic_sessions| topic_sessions.values().copied().collect())
                .unwrap_or_default()
        })
    }
}
