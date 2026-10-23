use super::*;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::{Arc, RwLock};

/// In-memory implementation of MailboxUrlStore.
pub struct MemMailboxUrlStore {
    data: RwLock<HashMap<MailboxId, String>>,
}

impl MemMailboxUrlStore {
    pub fn new() -> Self {
        Self {
            data: RwLock::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl MailboxUrlStore for MemMailboxUrlStore {
    async fn record_url(&self, id: &MailboxId, url: &str) -> anyhow::Result<()> {
        self.data
            .write()
            .unwrap()
            .insert(id.clone(), url.to_string());
        Ok(())
    }

    async fn get_url_inverse(&self, url: &str) -> anyhow::Result<Option<MailboxId>> {
        let data = self.data.read().unwrap();
        for (id, v) in data.iter() {
            if v == url {
                return Ok(Some(id.clone()));
            }
        }
        Ok(None)
    }

    async fn drop_mailbox(&self, id: &MailboxId) -> anyhow::Result<()> {
        self.data.write().unwrap().remove(id);
        Ok(())
    }
}

/// In-memory implementation of SyncStore.
pub struct MemSyncStore<T, A>
where
    T: mailbox_client::ItemTraits + 'static,
    A: mailbox_client::ItemTraits + 'static,
{
    status: RwLock<HashMap<MailboxId, SyncStatus>>,
    sync_state: RwLock<HashMap<MailboxId, HashMap<T, HashMap<A, mailbox_client::SeqNum>>>>,
}

impl<T, A> MemSyncStore<T, A>
where
    T: mailbox_client::ItemTraits + 'static,
    A: mailbox_client::ItemTraits + 'static,
{
    pub fn new() -> Self {
        Self {
            status: RwLock::new(HashMap::new()),
            sync_state: RwLock::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl<T, A> SyncStore<T, A> for MemSyncStore<T, A>
where
    T: mailbox_client::ItemTraits + 'static,
    A: mailbox_client::ItemTraits + 'static,
{
    async fn record_status(&self, id: &MailboxId, status: SyncStatus) -> anyhow::Result<()> {
        self.status.write().unwrap().insert(id.clone(), status);
        Ok(())
    }

    async fn get_status(&self, id: &MailboxId) -> anyhow::Result<Option<SyncStatus>> {
        Ok(self.status.read().unwrap().get(id).cloned())
    }

    async fn clear_status(&self, id: &MailboxId) -> anyhow::Result<()> {
        self.status.write().unwrap().remove(id);
        Ok(())
    }

    async fn drop_mailbox(&self, id: &MailboxId) -> anyhow::Result<()> {
        self.status.write().unwrap().remove(id);
        self.sync_state.write().unwrap().remove(id);
        Ok(())
    }

    async fn record_synced(
        &self,
        id: &MailboxId,
        watermarks: &[(T, A, mailbox_client::SeqNum)],
    ) -> anyhow::Result<()> {
        let mut state = self.sync_state.write().unwrap();
        let mailbox_state = state.entry(id.clone()).or_default();

        for (topic, author, seq) in watermarks {
            let author_state = mailbox_state.entry(*topic).or_default();
            let current = author_state.entry(*author).or_insert(0);
            *current = (*current).max(*seq);
        }
        Ok(())
    }

    async fn get_sync_state(
        &self,
        id: &MailboxId,
    ) -> anyhow::Result<HashMap<T, HashMap<A, mailbox_client::SeqNum>>> {
        let state = self.sync_state.read().unwrap();
        Ok(state.get(id).cloned().unwrap_or_default())
    }

    async fn get_watermark(
        &self,
        id: &MailboxId,
        topic: &T,
        author: &A,
    ) -> anyhow::Result<Option<mailbox_client::SeqNum>> {
        let state = self.sync_state.read().unwrap();
        Ok(state
            .get(id)
            .and_then(|mailbox_state| mailbox_state.get(topic))
            .and_then(|author_state| author_state.get(author))
            .copied())
    }

    async fn get_synced_for_log(
        &self,
        topic: &T,
        author: &A,
    ) -> anyhow::Result<BTreeMap<MailboxId, mailbox_client::SeqNum>> {
        let state = self.sync_state.read().unwrap();
        let mut results = BTreeMap::new();

        for (id, mailbox_state) in state.iter() {
            if let Some(author_state) = mailbox_state.get(topic) {
                if let Some(&seq) = author_state.get(author) {
                    results.insert(id.clone(), seq);
                }
            }
        }
        Ok(results)
    }

    async fn load_all_ids(&self) -> anyhow::Result<BTreeSet<MailboxId>> {
        let state = self.sync_state.read().unwrap();
        Ok(state.keys().cloned().collect())
    }
}

/// In-memory persistence implementation.
pub struct MemPersistence<T, A>
where
    T: mailbox_client::ItemTraits + 'static,
    A: mailbox_client::ItemTraits + 'static,
{
    mailbox_store: Arc<MemMailboxUrlStore>,
    sync_store: Arc<MemSyncStore<T, A>>,
}

impl<T, A> MemPersistence<T, A>
where
    T: mailbox_client::ItemTraits + 'static,
    A: mailbox_client::ItemTraits + 'static,
{
    pub fn new() -> Self {
        Self {
            mailbox_store: Arc::new(MemMailboxUrlStore::new()),
            sync_store: Arc::new(MemSyncStore::new()),
        }
    }
}

#[async_trait]
impl<T, A> Persistence<T, A> for MemPersistence<T, A>
where
    T: mailbox_client::ItemTraits + 'static,
    A: mailbox_client::ItemTraits + 'static,
{
    fn mailbox_store(&self) -> Arc<dyn MailboxUrlStore> {
        self.mailbox_store.clone()
    }

    fn sync_store(&self) -> Arc<dyn SyncStore<T, A>> {
        self.sync_store.clone()
    }

    async fn close(&self) {}
}
