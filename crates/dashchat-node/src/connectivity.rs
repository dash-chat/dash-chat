use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    sync::Arc,
};

use chrono::{DateTime, Duration, Utc};
use derive_more::derive::Constructor;
use mailbox_client::{MailboxId, manager::MailboxStatus};
use tokio::sync::RwLock;

use crate::{DeviceId, TopicId};

#[derive(Clone, Debug)]
pub struct Connectivity(Arc<RwLock<ConnectivityState>>);

#[derive(Debug)]
pub struct ConnectivityState {
    pub mailboxes: HashMap<MailboxId, MailboxConnectivity>,
    pub peers: HashMap<TopicId, HashMap<DeviceId, PeerConnectivity>>,
    pub config: ConnectivityConfig,
    last_pruned: DateTime<Utc>,
}

#[derive(Constructor, Debug)]
pub struct MailboxConnectivity {
    pub status: MailboxStatus,
    pub last_updated: DateTime<Utc>,
}

#[derive(Constructor, Debug)]
pub struct PeerConnectivity {
    pub last_updated: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct ConnectivityConfig {
    pub stale_duration: Duration,
    pub prune_interval: Duration,
}

impl Default for ConnectivityConfig {
    fn default() -> Self {
        Self {
            stale_duration: Duration::minutes(3),
            prune_interval: Duration::minutes(1),
        }
    }
}

impl Connectivity {
    pub fn new(config: ConnectivityConfig) -> Self {
        Self(Arc::new(RwLock::new(ConnectivityState::new(config))))
    }

    pub async fn update(&self, update: ConnectivityUpdate) {
        self.0.write().await.update(update);
    }

    pub async fn report(&self, topic: TopicId) -> ConnectivityReport {
        self.0.write().await.report(topic)
    }
}

impl ConnectivityState {
    fn new(config: ConnectivityConfig) -> Self {
        Self {
            config,
            mailboxes: HashMap::new(),
            peers: HashMap::new(),
            last_pruned: Utc::now(),
        }
    }

    fn update(&mut self, update: ConnectivityUpdate) {
        match update {
            ConnectivityUpdate::Mailbox(mailbox_id, status) => {
                self.mailboxes
                    .insert(mailbox_id, MailboxConnectivity::new(status, Utc::now()));
            }
            ConnectivityUpdate::Peer {
                topic_id,
                device_id,
                success,
            } => {
                if success {
                    self.peers
                        .entry(topic_id)
                        .or_default()
                        .insert(device_id, PeerConnectivity::new(Utc::now()));
                }
            }
        }
    }

    fn report(&mut self, topic: TopicId) -> ConnectivityReport {
        self.prune();
        ConnectivityReport {
            mailboxes: self
                .mailboxes
                .iter()
                .map(|(mailbox_id, mailbox_connectivity)| {
                    (mailbox_id.clone(), mailbox_connectivity.status)
                })
                .collect(),
            peers: self
                .peers
                .get(&topic)
                .map(|peers| peers.keys().cloned().collect())
                .unwrap_or_default(),
        }
    }

    fn prune(&mut self) {
        let now = Utc::now();
        if now.signed_duration_since(self.last_pruned) < self.config.prune_interval {
            return;
        }
        self.mailboxes.retain(|_, mailbox_connectivity| {
            now.signed_duration_since(mailbox_connectivity.last_updated)
                < self.config.stale_duration
        });
        self.peers.retain(|_, peers| {
            peers.retain(|_, peer_connectivity| {
                now.signed_duration_since(peer_connectivity.last_updated)
                    < self.config.stale_duration
            });
            !peers.is_empty()
        });
        self.last_pruned = now;
    }
}

pub enum ConnectivityUpdate {
    Mailbox(MailboxId, MailboxStatus),
    Peer {
        topic_id: TopicId,
        device_id: DeviceId,
        success: bool,
    },
}

/// A report on all connected nodes for a given topic.
#[derive(Clone, Debug)]
pub struct ConnectivityReport {
    /// Mailboxes are not topic-specific, so any recently synced mailboxes are returned here.
    pub mailboxes: BTreeMap<MailboxId, MailboxStatus>,
    /// Peers who we have synced with on a given topic are returned here.
    pub peers: BTreeSet<DeviceId>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn topic(byte: u8) -> TopicId {
        TopicId::from([byte; 32])
    }

    fn device(byte: u8) -> DeviceId {
        use p2panda::SigningKey;
        DeviceId::from(SigningKey::from_bytes(&[byte; 32]).verifying_key())
    }

    fn instant_prune_config() -> ConnectivityConfig {
        ConnectivityConfig {
            stale_duration: Duration::minutes(3),
            prune_interval: Duration::zero(),
        }
    }

    fn state_with_config(config: ConnectivityConfig) -> ConnectivityState {
        let mut s = ConnectivityState::new(config);
        s.last_pruned = DateTime::UNIX_EPOCH;
        s
    }

    #[test]
    fn stale_peer_is_pruned_from_topic() {
        let mut state = state_with_config(instant_prune_config());
        let t = topic(1);
        let d = device(1);

        state
            .peers
            .entry(t)
            .or_default()
            .insert(d, PeerConnectivity::new(Utc::now() - Duration::minutes(10)));

        let report = state.report(t);
        assert!(report.peers.is_empty());
    }

    #[test]
    fn fresh_peer_survives_while_stale_peer_is_removed() {
        let mut state = state_with_config(instant_prune_config());
        let t = topic(1);
        let stale = device(1);
        let fresh = device(2);

        let peers = state.peers.entry(t).or_default();
        peers.insert(
            stale,
            PeerConnectivity::new(Utc::now() - Duration::minutes(10)),
        );
        peers.insert(fresh, PeerConnectivity::new(Utc::now()));

        let report = state.report(t);
        assert!(!report.peers.contains(&stale));
        assert!(report.peers.contains(&fresh));
    }

    #[test]
    fn topic_entry_dropped_when_all_peers_stale() {
        let mut state = state_with_config(instant_prune_config());
        let t = topic(1);

        state.peers.entry(t).or_default().insert(
            device(1),
            PeerConnectivity::new(Utc::now() - Duration::minutes(10)),
        );

        state.report(t);
        assert!(!state.peers.contains_key(&t));
    }

    #[test]
    fn failed_sync_does_not_add_peer() {
        let mut state = ConnectivityState::new(instant_prune_config());
        let t = topic(1);
        let d = device(1);

        state.update(ConnectivityUpdate::Peer {
            topic_id: t,
            device_id: d,
            success: false,
        });

        assert!(state.peers.get(&t).map_or(true, |p| !p.contains_key(&d)));
    }

    #[test]
    fn prune_interval_gates_pruning() {
        let config = ConnectivityConfig {
            stale_duration: Duration::minutes(3),
            prune_interval: Duration::hours(1),
        };
        let mut state = ConnectivityState::new(config);
        let t = topic(1);
        let d = device(1);

        state
            .peers
            .entry(t)
            .or_default()
            .insert(d, PeerConnectivity::new(Utc::now() - Duration::minutes(10)));

        let report = state.report(t);
        assert!(
            report.peers.contains(&d),
            "stale peer should survive when prune interval has not elapsed"
        );
    }
}
