use std::collections::BTreeMap;

use crate::MailboxItem;

#[async_trait::async_trait]
pub trait MailboxStore<Item: MailboxItem>: Clone + Send + Sync + 'static {
    // async fn store_blob(&self, blob: Blob) -> Result<(), anyhow::Error>;

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
}
