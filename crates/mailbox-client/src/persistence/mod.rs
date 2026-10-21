use crate::MailboxId;
use crate::connection_health::SyncStatus;
use async_trait::async_trait;
use std::collections::BTreeSet;
use std::sync::Arc;

pub mod mem;
pub mod sqlite;

/// Repository for mailbox identity and location.
#[async_trait]
pub trait MailboxUrlStore: Send + Sync {
    async fn record_url(&self, id: &MailboxId, url: &str) -> anyhow::Result<()>;
    async fn get_url(&self, id: &MailboxId) -> anyhow::Result<Option<String>>;
    async fn get_url_inverse(&self, url: &str) -> anyhow::Result<Option<MailboxId>>;
    async fn drop_mailbox(&self, id: &MailboxId) -> anyhow::Result<()>;
}

/// Repository for mailbox sync progress and status.
#[async_trait]
pub trait SyncStore<T, A>: Send + Sync
where
    T: ByteSerializable,
    A: ByteSerializable,
{
    async fn record_status(&self, id: &MailboxId, status: SyncStatus) -> anyhow::Result<()>;
    async fn get_status(&self, id: &MailboxId) -> anyhow::Result<Option<SyncStatus>>;
    async fn clear_status(&self, id: &MailboxId) -> anyhow::Result<()>;
    async fn drop_mailbox(&self, id: &MailboxId) -> anyhow::Result<()>;
    async fn record_synced(
        &self,
        id: &MailboxId,
        watermarks: &[(T, A, crate::SeqNum)],
    ) -> anyhow::Result<()>;
    async fn get_sync_state(
        &self,
        id: &MailboxId,
    ) -> anyhow::Result<std::collections::HashMap<T, std::collections::HashMap<A, crate::SeqNum>>>;
    async fn get_watermark(
        &self,
        id: &MailboxId,
        topic: &T,
        author: &A,
    ) -> anyhow::Result<Option<crate::SeqNum>>;
    async fn get_synced_for_log(
        &self,
        topic: &T,
        author: &A,
    ) -> anyhow::Result<std::collections::BTreeMap<MailboxId, crate::SeqNum>>;
    async fn load_all_ids(&self) -> anyhow::Result<BTreeSet<MailboxId>>;
}

/// Unified persistence manager that provides access to the underlying stores.
pub trait Persistence<T, A>: Send + Sync
where
    T: ByteSerializable,
    A: ByteSerializable,
{
    fn mailbox_store(&self) -> Arc<dyn MailboxUrlStore>;
    fn sync_store(&self) -> Arc<dyn SyncStore<T, A>>;
}

/// Repositories for types that can be serialized to bytes.
pub trait ByteSerializable: Send + Sync + Eq + std::hash::Hash + Clone + 'static {
    fn encode(&self) -> anyhow::Result<Vec<u8>>;
}

impl<T> ByteSerializable for T
where
    T: serde::Serialize + Send + Sync + Eq + std::hash::Hash + Clone + 'static,
{
    fn encode(&self) -> anyhow::Result<Vec<u8>> {
        crate::sync_tracker::encode(self)
    }
}

pub trait ByteDeserializable<T>: Send + Sync {
    fn decode(bytes: &[u8]) -> anyhow::Result<T>;
}

impl<T> ByteDeserializable<T> for T
where
    T: serde::de::DeserializeOwned + Send + Sync + Eq + std::hash::Hash + Clone + 'static,
{
    fn decode(bytes: &[u8]) -> anyhow::Result<T> {
        crate::sync_tracker::decode(bytes)
    }
}
