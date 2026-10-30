use super::*;
use mailbox_client::{ItemTraits, SeqNum};
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
    T: ItemTraits + 'static,
    A: ItemTraits + 'static,
{
    status: RwLock<HashMap<MailboxId, SyncStatus>>,
    sync_state: RwLock<HashMap<MailboxId, HashMap<T, HashMap<A, SeqNum>>>>,
    blob_sync_state: RwLock<HashMap<MailboxId, HashMap<T, HashMap<A, SeqNum>>>>,
}

impl<T, A> MemSyncStore<T, A>
where
    T: ItemTraits + 'static,
    A: ItemTraits + 'static,
{
    pub fn new() -> Self {
        Self {
            status: RwLock::new(HashMap::new()),
            sync_state: RwLock::new(HashMap::new()),
            blob_sync_state: RwLock::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl<T, A> SyncStore<T, A> for MemSyncStore<T, A>
where
    T: ItemTraits + 'static,
    A: ItemTraits + 'static,
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
        self.blob_sync_state.write().unwrap().remove(id);
        Ok(())
    }

    async fn record_synced(
        &self,
        id: &MailboxId,
        watermarks: &[(T, A, SeqNum)],
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
    ) -> anyhow::Result<HashMap<T, HashMap<A, SeqNum>>> {
        let state = self.sync_state.read().unwrap();
        Ok(state.get(id).cloned().unwrap_or_default())
    }

    async fn get_watermark(
        &self,
        id: &MailboxId,
        topic: &T,
        author: &A,
    ) -> anyhow::Result<Option<SeqNum>> {
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
    ) -> anyhow::Result<BTreeMap<MailboxId, SeqNum>> {
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

    async fn record_blob_synced(
        &self,
        id: &MailboxId,
        heights: &[(T, A, SeqNum)],
    ) -> anyhow::Result<()> {
        let mut state = self.blob_sync_state.write().unwrap();
        let mailbox_state = state.entry(id.clone()).or_default();
        for (topic, author, seq) in heights {
            let author_state = mailbox_state.entry(*topic).or_default();
            let current = author_state.entry(*author).or_insert(0);
            *current = (*current).max(*seq);
        }
        Ok(())
    }

    async fn get_blob_sync_state(
        &self,
        id: &MailboxId,
    ) -> anyhow::Result<HashMap<T, HashMap<A, SeqNum>>> {
        let state = self.blob_sync_state.read().unwrap();
        Ok(state.get(id).cloned().unwrap_or_default())
    }
}

/// In-memory persistence implementation.
pub struct MemPersistence<T, A>
where
    T: ItemTraits + 'static,
    A: ItemTraits + 'static,
{
    mailbox_store: Arc<MemMailboxUrlStore>,
    sync_store: Arc<MemSyncStore<T, A>>,
    blob_store: Arc<MemBlobStore>,
}

impl<T, A> MemPersistence<T, A>
where
    T: ItemTraits + 'static,
    A: ItemTraits + 'static,
{
    pub fn new() -> Self {
        Self {
            mailbox_store: Arc::new(MemMailboxUrlStore::new()),
            sync_store: Arc::new(MemSyncStore::new()),
            blob_store: Arc::new(MemBlobStore::default()),
        }
    }
}

#[async_trait]
impl<T, A> Persistence<T, A> for MemPersistence<T, A>
where
    T: ItemTraits + 'static,
    A: ItemTraits + 'static,
{
    fn mailbox_store(&self) -> Arc<dyn MailboxUrlStore> {
        self.mailbox_store.clone()
    }

    fn sync_store(&self) -> Arc<dyn SyncStore<T, A>> {
        self.sync_store.clone()
    }

    fn blob_store(&self) -> Arc<dyn BlobStore> {
        self.blob_store.clone()
    }

    async fn close(&self) {}
}

/// In-memory blob store for tests.
#[derive(Default)]
pub struct MemBlobStore {
    data: RwLock<BTreeMap<MailboxId, BTreeSet<iroh_blobs::Hash>>>,
}

#[async_trait::async_trait]
impl BlobStore for MemBlobStore {
    async fn record(&self, id: &MailboxId, hashes: &[iroh_blobs::Hash]) -> anyhow::Result<()> {
        let mut data = self.data.write().unwrap();
        data.entry(id.clone())
            .or_default()
            .extend(hashes.iter().copied());
        Ok(())
    }

    async fn remove(&self, id: &MailboxId, hashes: &[iroh_blobs::Hash]) -> anyhow::Result<()> {
        let mut data = self.data.write().unwrap();
        if let Some(set) = data.get_mut(id) {
            for hash in hashes {
                set.remove(hash);
            }
            if set.is_empty() {
                data.remove(id);
            }
        }
        Ok(())
    }

    async fn unfetched(&self, id: &MailboxId) -> anyhow::Result<Vec<iroh_blobs::Hash>> {
        Ok(self
            .data
            .read()
            .unwrap()
            .get(id)
            .map(|set| set.iter().copied().collect())
            .unwrap_or_default())
    }

    async fn unfetched_ids(&self) -> anyhow::Result<BTreeSet<MailboxId>> {
        Ok(self.data.read().unwrap().keys().cloned().collect())
    }

    async fn remove_all_mailboxes(&self, hashes: &[iroh_blobs::Hash]) -> anyhow::Result<()> {
        let mut data = self.data.write().unwrap();
        for set in data.values_mut() {
            for hash in hashes {
                set.remove(hash);
            }
        }
        data.retain(|_, set| !set.is_empty());
        Ok(())
    }
}
