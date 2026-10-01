pub mod backends;
pub mod manager;
pub mod store;
pub mod sync_tracker;
pub mod unfetched_blobs;
pub mod upload_scheduler;

pub use unfetched_blobs::BlobSource;

pub use mailbox_server::RegisterPeerRequest;

#[cfg(test)]
pub mod testing;

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    sync::Arc,
    time::Duration,
};

use once_cell::sync::Lazy;
use tokio::sync::{Mutex, mpsc};
use tracing::Instrument;

use serde::{Deserialize, Serialize, de::DeserializeOwned};

pub static HTTP_CLIENT: Lazy<reqwest::Client> = Lazy::new(|| {
    // Setting up a connection on a loaded mobile network alone can take
    // several seconds, so the connect budget matches Signal's 15 s. The total
    // covers the connect too.
    let builder = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(20));
    // The e2e mailbox serves TLS so that a degraded link slows the handshake,
    // which the connect timeout covers, as a real network does. DER because
    // native-tls only parses PEM on macOS and panics on iOS.
    #[cfg(feature = "e2e-test-ca")]
    let builder = builder.add_root_certificate(
        reqwest::Certificate::from_der(include_bytes!("../e2e-test-ca/ca.der"))
            .expect("e2e test CA is valid DER"),
    );
    builder.build().expect("Failed to build HTTP client")
});

#[async_trait::async_trait]
pub trait MailboxClient<Item: MailboxItem>: Send + Sync + 'static {
    fn id(&self) -> MailboxId;

    /// The base URL this client talks to, if it has one.
    fn url(&self) -> Option<String> {
        None
    }

    /// Publish operations to the mailbox during topic sync.
    /// Different mailbox implementations have different semantics for this,
    /// for instance separate storage for logs vs blobs.
    async fn publish(&self, ops: Vec<Item>) -> Result<PublishResponse<Item>, anyhow::Error>;

    /// Fetch operations from the mailbox for the given topics.
    ///
    /// The inner map associated each author with the height of their locally stored log.
    /// The height represents the highest sequence number stored for that author, meaning that the mailbox
    /// should only return operations with a higher sequence for that author.
    /// NOTE that this is a subtractive, not additive, filter, meaning that any authors not included
    /// in the `min_heights` list will have their *entire* log returned, including if `min_heights` is empty.
    /// This is so that the mailbox is used for author discovery as well.
    /// The intention is that all data is encrypted and only decipherable by valid recipients.
    async fn fetch(
        &self,
        request: FetchRequest<Item>,
    ) -> Result<FetchResponse<Item>, anyhow::Error>;

    /// Report one or more devices to this mailbox's `/report` endpoint. The
    /// default no-op covers in-memory/test mailboxes that have no server to
    /// receive reports.
    async fn report(&self, _request: reporting::ReportRequest) -> Result<(), anyhow::Error> {
        Ok(())
    }

    /// Push unfetched blob bytes to the mailbox, best-effort.
    ///
    /// The default no-op covers in-memory/test mailboxes that do not store
    /// blobs. Implementations should announce the hashes to the mailbox and
    /// stream the bytes for the ones it lacks.
    async fn push_blobs(
        &self,
        _hashes: Vec<iroh_blobs::Hash>,
        _reader: Arc<dyn BlobReader>,
        _tracker: Arc<dyn UnfetchedBlobTracker>,
    ) -> Result<(), anyhow::Error> {
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(bound(deserialize = "Item: DeserializeOwned"))]
pub struct FetchRequest<Item: MailboxItem>(pub BTreeMap<Item::Topic, FetchTopicRequest<Item>>);

pub type FetchTopicRequest<Item> = BTreeMap<<Item as MailboxItem>::Author, u64>;

/// Returned by the `fetch` method.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(bound(deserialize = "Item: DeserializeOwned"))]
pub struct FetchResponse<Item: MailboxItem>(pub BTreeMap<Item::Topic, FetchTopicResponse<Item>>);

/// Returned by the `publish` method: for each log in the request, the mailbox's
/// resulting contiguity watermark (`None` when none could be established).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublishResponse<Item: MailboxItem>(
    pub BTreeMap<Item::Topic, BTreeMap<Item::Author, Option<SeqNum>>>,
);

impl<Item: MailboxItem> Default for PublishResponse<Item> {
    fn default() -> Self {
        Self(BTreeMap::new())
    }
}

impl<Item: MailboxItem> PublishResponse<Item> {
    pub fn watermark(&self, topic: &Item::Topic, author: &Item::Author) -> Option<SeqNum> {
        self.0
            .get(topic)
            .and_then(|m| m.get(author))
            .copied()
            .flatten()
    }
}

/// Returned by the `fetch` method.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(bound(deserialize = "Item: DeserializeOwned"))]
pub struct FetchTopicResponse<Item: MailboxItem> {
    /// The operations not held locally that were fetched.
    pub items: Vec<Item>,
    /// The operations held locally that are missing from the mailbox,
    /// and which this node should now publish.
    pub missing: HashMap<<Item as MailboxItem>::Author, Vec<u64>>,
}

pub type MailboxId = String;
pub type SeqNum = u64;

pub trait ItemTraits:
    Copy + Eq + Ord + std::hash::Hash + std::fmt::Debug + Serialize + DeserializeOwned + Send + Sync
{
}

impl<T> ItemTraits for T where
    T: Copy
        + Eq
        + Ord
        + std::hash::Hash
        + std::fmt::Debug
        + Serialize
        + DeserializeOwned
        + Send
        + Sync
{
}

/// How a `Topic` or `Author` maps to a string key for addressing a log inside
/// a mailbox.
///
/// This key is persisted server-side and shared across client versions, so the
/// encoding for any type already in production must never change. The concrete
/// mailbox-server (`mailbox-server`) places additional constraints on keys:
/// they must not contain `:` or NUL, and topic keys used for push notifications
/// must pass a 64-character lowercase-hex validation (`validate_hex32`). The
/// production implementations (`TopicId` below and `DeviceId` in `dashchat-node`)
/// satisfy those constraints; test-only fixtures (e.g. `u8`/`char` for
/// `crate::testing::Msg`) are intentionally minimal and are not sent to a real
/// mailbox.
pub trait MailboxKey: ItemTraits {
    fn to_mailbox_key(&self) -> String;
    fn from_mailbox_key(key: &str) -> Result<Self, anyhow::Error>;
}

pub trait MailboxItem:
    Clone + std::fmt::Debug + Serialize + DeserializeOwned + Send + Sync + 'static
{
    type Hash: ItemTraits;
    type Author: MailboxKey;
    type Topic: MailboxKey;

    fn seq_num(&self) -> SeqNum;
    fn hash(&self) -> Self::Hash;
    fn author(&self) -> Self::Author;
    fn topic(&self) -> Self::Topic;
    fn blob_hashes(&self) -> Vec<iroh_blobs::Hash> {
        Vec::new()
    }
}

/// Extra traits for ItemTraits which are feature-dependent.
pub trait OptionalItemTraits {}
impl<T> OptionalItemTraits for T {}

/// Node-side sink for per-mailbox unfetched blob-hash tracking. Implemented in
/// `dashchat-node` over `LocalStore`; kept as a trait here so this crate stays
/// free of node types.
#[async_trait::async_trait]
pub trait UnfetchedBlobTracker: Send + Sync + 'static {
    async fn record(&self, mailbox_id: &MailboxId, hashes: &[iroh_blobs::Hash]);
    async fn remove(&self, mailbox_id: &MailboxId, hashes: &[iroh_blobs::Hash]);
}

/// Node-side source of blob bytes by hash. Implemented in `dashchat-node` over
/// the node's blob store; kept as a trait here so this crate stays free of node
/// types. Used by the toy client to upload blob bytes inline to a mailbox.
#[async_trait::async_trait]
pub trait BlobReader: Send + Sync + 'static {
    async fn read_blob(&self, hash: iroh_blobs::Hash) -> anyhow::Result<bytes::Bytes>;
    async fn has_blob(&self, hash: iroh_blobs::Hash) -> bool;
}

/// No-op tracker for tests and contexts that don't persist unfetched blobs.
#[derive(Clone, Default)]
pub struct NoopUnfetchedBlobTracker;

#[async_trait::async_trait]
impl UnfetchedBlobTracker for NoopUnfetchedBlobTracker {
    async fn record(&self, _mailbox_id: &MailboxId, _hashes: &[iroh_blobs::Hash]) {}
    async fn remove(&self, _mailbox_id: &MailboxId, _hashes: &[iroh_blobs::Hash]) {}
}

impl MailboxKey for p2panda_core::Topic {
    fn to_mailbox_key(&self) -> String {
        self.to_hex()
    }

    fn from_mailbox_key(key: &str) -> Result<Self, anyhow::Error> {
        Ok(key.parse()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn topic_id_round_trips_through_mailbox_key() {
        let bytes = [0xab; 32];
        let topic = p2panda_core::Topic::from(bytes);
        let key = topic.to_mailbox_key();
        assert_eq!(
            key,
            "abababababababababababababababababababababababababababababababab"
        );
        assert_eq!(p2panda_core::Topic::from_mailbox_key(&key).unwrap(), topic);
    }

    #[test]
    fn topic_id_from_mailbox_key_rejects_non_hex() {
        assert!(p2panda_core::Topic::from_mailbox_key("not-hex").is_err());
    }

    #[test]
    fn topic_id_from_mailbox_key_rejects_wrong_length() {
        assert!(p2panda_core::Topic::from_mailbox_key("ab").is_err());
        assert!(
            p2panda_core::Topic::from_mailbox_key(
                "ababababababababababababababababababababababababababababababababcd"
            )
            .is_err()
        );
    }
}
