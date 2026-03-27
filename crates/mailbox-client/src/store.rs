use std::collections::BTreeMap;

use crate::{MailboxItem, Opaq, OpaqHash};

/// The interface for locally storing and retrieving log information for syncing with a mailbox.
pub trait LocalMailboxStore<Item: MailboxItem>:
    LocalMailboxLogStore<Item> + LocalMailboxOpaqStore
{
}
impl<T, I> LocalMailboxStore<I> for T
where
    I: MailboxItem,
    T: LocalMailboxLogStore<I> + LocalMailboxOpaqStore,
{
}

#[async_trait::async_trait]
pub trait LocalMailboxLogStore<Item: MailboxItem>: Send + Sync + 'static {
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

/// The interface for remotely storing and retrieving blobs from a mailbox server.
#[async_trait::async_trait]
pub trait LocalMailboxOpaqStore: Send + Sync + 'static {
    async fn has_mailbox_opaq(&self, hash: OpaqHash) -> anyhow::Result<bool>;
    async fn get_mailbox_opaq(&self, hash: OpaqHash) -> anyhow::Result<Option<Opaq>>;
    async fn store_mailbox_opaq(&self, blob: Opaq) -> anyhow::Result<()>;
}
