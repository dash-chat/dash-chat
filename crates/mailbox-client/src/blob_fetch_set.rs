use std::{collections::HashMap, sync::Arc};

use tokio::sync::Mutex;

use crate::{MailboxId, OpaqHash};

/// Tracks blob hashes that could not be fetched during sync, for retry on subsequent polls.
///
/// Each entry records the preferred source mailbox (where the blob was first expected).
/// During retry, all currently registered mailboxes are tried, with the preferred one first.
#[derive(Clone, Default)]
pub struct BlobFetchSet {
    inner: Arc<Mutex<HashMap<OpaqHash, MailboxId>>>,
}

impl BlobFetchSet {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a blob hash to the set. If the hash is already present the preferred mailbox is updated.
    pub async fn insert(&self, blob_hash: OpaqHash, preferred_mailbox: MailboxId) {
        self.inner.lock().await.insert(blob_hash, preferred_mailbox);
    }

    /// Remove a blob hash from the set (called after a successful fetch).
    pub async fn remove(&self, blob_hash: &OpaqHash) {
        self.inner.lock().await.remove(blob_hash);
    }

    pub async fn pending(&self) -> Vec<(OpaqHash, MailboxId)> {
        self.inner
            .lock()
            .await
            .iter()
            .map(|(h, m)| (*h, m.clone()))
            .collect()
    }

    pub async fn is_empty(&self) -> bool {
        self.inner.lock().await.is_empty()
    }
}
