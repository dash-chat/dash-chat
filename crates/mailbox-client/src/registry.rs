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

    /// Looks up an existing mailbox under `id`; if one is present, `f` is called
    /// with the existing handle and the returned `TrackedMailbox` is ignored.
    /// If no mailbox is present, `f` is called with a fresh `TrackedMailbox`
    /// built from `create`, and the result is inserted under `id`.
    ///
    /// Returns the mailbox stored under `id` after the operation, and whether it
    /// was freshly inserted.
    pub async fn get_or_insert_with<F>(
        &self,
        id: MailboxId,
        create: F,
    ) -> (Arc<TrackedMailbox<Item>>, bool)
    where
        F: FnOnce() -> TrackedMailbox<Item>,
    {
        let mut map = self.mailboxes.lock().await;
        if let Some(existing) = map.get(&id).cloned() {
            return (existing, false);
        }

        let mailbox = Arc::new(create());
        map.insert(id.clone(), mailbox.clone());
        self.send_active_ids(&map);
        (mailbox, true)
    }

    /// Removes the mailbox for `id`, if any.
    ///
    /// Returns `true` when an entry was removed.
    pub async fn unregister(&self, id: &MailboxId) -> bool {
        let mut map = self.mailboxes.lock().await;
        if map.remove(id).is_some() {
            self.send_active_ids(&map);
            true
        } else {
            false
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

    fn send_active_ids(&self, map: &BTreeMap<MailboxId, Arc<TrackedMailbox<Item>>>) {
        let active: BTreeSet<MailboxId> = map.keys().cloned().collect();
        let _ = self.active_mailbox_ids_tx.send_replace(active);
    }
}
