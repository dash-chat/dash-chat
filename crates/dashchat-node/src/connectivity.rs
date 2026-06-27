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

impl Default for MailboxConnectivity {
    fn default() -> Self {
        Self {
            status: MailboxStatus::Stopped,
            last_updated: Utc::now(),
        }
    }
}

#[derive(Constructor, Default, Debug)]
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
                    self.peers.entry(topic_id).or_insert_with(HashMap::new);
                    self.peers
                        .get_mut(&topic_id)
                        .unwrap()
                        .entry(device_id)
                        .insert_entry(PeerConnectivity::new(Utc::now()));
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
            peers.values().any(|peer_connectivity| {
                now.signed_duration_since(peer_connectivity.last_updated)
                    < self.config.stale_duration
            })
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

#[derive(Clone, Debug)]
pub struct ConnectivityReport {
    pub mailboxes: BTreeMap<MailboxId, MailboxStatus>,
    pub peers: BTreeSet<DeviceId>,
}
