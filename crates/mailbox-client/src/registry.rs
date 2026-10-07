use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use tokio::sync::{Mutex, watch};

use super::manager::TrackedMailbox;
use super::{MailboxId, MailboxItem};

/// Internal registry for managing a collection of tracked mailboxes.
pub struct Registry<Item: MailboxItem> {
    pub(crate) mailboxes: Arc<Mutex<BTreeMap<MailboxId, Arc<TrackedMailbox<Item>>>>>,
    active_mailbox_ids_tx: watch::Sender<BTreeSet<MailboxId>>,
}

impl<Item: MailboxItem> Registry<Item> {
    pub fn new() -> Self {
        let (tx, _) = watch::channel(BTreeSet::new());
        Self {
            mailboxes: Arc::new(Mutex::new(BTreeMap::new())),
            active_mailbox_ids_tx: tx,
        }
    }

    pub async fn register(&self, id: MailboxId, mailbox: TrackedMailbox<Item>) {
        let mut map = self.mailboxes.lock().await;
        let arc_mailbox = Arc::new(mailbox);
        map.insert(id.clone(), arc_mailbox);

        let mut active = BTreeSet::new();
        for (mid, _) in map.iter() {
            active.insert(mid.clone());
        }
        let _ = self.active_mailbox_ids_tx.send_replace(active);
    }

    pub async fn unregister(&self, id: &MailboxId) {
        let mut map = self.mailboxes.lock().await;
        if map.remove(id).is_some() {
            let mut active = BTreeSet::new();
            for (mid, _) in map.iter() {
                active.insert(mid.clone());
            }
            let _ = self.active_mailbox_ids_tx.send_replace(active);
        }
    }

    pub async fn clear(&self) {
        let mut map = self.mailboxes.lock().await;
        map.clear();
        let _ = self.active_mailbox_ids_tx.send(BTreeSet::new());
    }

    pub async fn get(&self, id: &MailboxId) -> Option<Arc<TrackedMailbox<Item>>> {
        self.mailboxes.lock().await.get(id).cloned()
    }

    pub async fn all(&self) -> Vec<(MailboxId, Arc<TrackedMailbox<Item>>)> {
        self.mailboxes
            .lock()
            .await
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    pub fn active_ids_rx(&self) -> watch::Receiver<BTreeSet<MailboxId>> {
        self.active_mailbox_ids_tx.subscribe()
    }
}
