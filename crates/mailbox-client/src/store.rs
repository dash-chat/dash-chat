use std::collections::BTreeMap;

use crate::{MailboxItem, Opaq, OpaqHash};

#[async_trait::async_trait]
pub trait MailboxStore<Item: MailboxItem>: Clone + Send + Sync + 'static {
    async fn get_log(
        &self,
        author: &Item::Author,
        topic: &Item::Topic,
        from: u64,
    ) -> Result<Option<Vec<Item>>, anyhow::Error>;

    async fn get_log_heights(
        &self,
        topic: &Item::Topic,
    ) -> Result<BTreeMap<Item::Author, u64>, anyhow::Error>;

    /// Retrieve a blob from local storage by its hash.
    /// Used during sync to publish locally-held blobs to remote mailboxes.
    async fn get_blob(&self, hash: &OpaqHash) -> Result<Option<Opaq>, anyhow::Error>;

    /// Store a blob fetched from a remote mailbox into local storage.
    async fn store_blob(&self, blob: Opaq) -> Result<(), anyhow::Error>;
}
