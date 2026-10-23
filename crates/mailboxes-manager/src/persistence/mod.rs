use async_trait::async_trait;
use mailbox_client::MailboxId;
use mailbox_client::SyncStatus;
use std::collections::BTreeSet;
use std::sync::Arc;

#[cfg(test)]
pub mod mem;
pub mod sqlite;

/// Repository for mailbox identity and location.
#[async_trait]
pub trait MailboxUrlStore: Send + Sync {
    async fn record_url(&self, id: &MailboxId, url: &str) -> anyhow::Result<()>;
    async fn get_url_inverse(&self, url: &str) -> anyhow::Result<Option<MailboxId>>;
    async fn drop_mailbox(&self, id: &MailboxId) -> anyhow::Result<()>;
}

/// Repository for mailbox sync progress and status.
#[async_trait]
pub trait SyncStore<T, A>: Send + Sync
where
    T: mailbox_client::ItemTraits + 'static,
    A: mailbox_client::ItemTraits + 'static,
{
    async fn record_status(&self, id: &MailboxId, status: SyncStatus) -> anyhow::Result<()>;
    async fn get_status(&self, id: &MailboxId) -> anyhow::Result<Option<SyncStatus>>;
    async fn clear_status(&self, id: &MailboxId) -> anyhow::Result<()>;
    async fn drop_mailbox(&self, id: &MailboxId) -> anyhow::Result<()>;
    async fn record_synced(
        &self,
        id: &MailboxId,
        watermarks: &[(T, A, mailbox_client::SeqNum)],
    ) -> anyhow::Result<()>;
    async fn get_sync_state(
        &self,
        id: &MailboxId,
    ) -> anyhow::Result<
        std::collections::HashMap<T, std::collections::HashMap<A, mailbox_client::SeqNum>>,
    >;
    async fn get_watermark(
        &self,
        id: &MailboxId,
        topic: &T,
        author: &A,
    ) -> anyhow::Result<Option<mailbox_client::SeqNum>>;
    async fn get_synced_for_log(
        &self,
        topic: &T,
        author: &A,
    ) -> anyhow::Result<std::collections::BTreeMap<MailboxId, mailbox_client::SeqNum>>;
    async fn load_all_ids(&self) -> anyhow::Result<BTreeSet<MailboxId>>;
}

/// Unified persistence manager that provides access to the underlying stores.
#[async_trait]
pub trait Persistence<T, A>: Send + Sync
where
    T: mailbox_client::ItemTraits + 'static,
    A: mailbox_client::ItemTraits + 'static,
{
    fn mailbox_store(&self) -> Arc<dyn MailboxUrlStore>;
    fn sync_store(&self) -> Arc<dyn SyncStore<T, A>>;

    /// Release the underlying storage (e.g. the SQLite pool and its file
    /// lock). No-op for in-memory persistence.
    async fn close(&self);
}

pub(crate) fn encode<T: serde::Serialize>(value: &T) -> anyhow::Result<Vec<u8>> {
    let mut buf = Vec::new();
    ciborium::into_writer(value, &mut buf)?;
    Ok(buf)
}

pub(crate) fn decode<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> anyhow::Result<T> {
    Ok(ciborium::from_reader(bytes)?)
}
