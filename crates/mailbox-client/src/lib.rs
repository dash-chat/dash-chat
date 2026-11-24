pub mod backends;
pub mod blobs;
mod health;
pub mod operations;
pub mod store;
pub mod sync_scheduler;
pub mod topic_registry;

pub use health::{LastError, MailboxConnectionHealth, MailboxesConfig, SyncStatus};

pub use blobs::iroh_push::IrohPusher;
pub use blobs::upload_lifecycle::BlobUploadLifecycle;
pub use mailbox_server::RegisterPeerRequest;

/// Test helpers and fixtures. Available outside `#[cfg(test)]` so dependent
/// crates' tests can reuse utilities like `noop_upload_lifecycle`.
#[cfg(feature = "testing")]
pub mod testing;

use std::collections::{BTreeMap, HashMap};

use once_cell::sync::Lazy;

use serde::{Deserialize, Serialize, de::DeserializeOwned};

pub static HTTP_CLIENT: Lazy<netwatch_http_client::Client> = Lazy::new(|| {
    netwatch_http_client::Client::new(configure_http_client).expect("Failed to build HTTP client")
});

fn configure_http_client() -> reqwest::ClientBuilder {
    let builder = reqwest::Client::builder()
        .connect_timeout(dashchat_utils::HTTP_CONNECT_TIMEOUT)
        .timeout(dashchat_utils::HTTP_REQUEST_TIMEOUT);
    // The e2e mailbox serves TLS so that a degraded link slows the handshake,
    // which the connect timeout covers, as a real network does. DER because
    // native-tls only parses PEM on macOS and panics on iOS.
    #[cfg(feature = "e2e-test-ca")]
    let builder = builder.add_root_certificate(
        reqwest::Certificate::from_der(include_bytes!("../e2e-test-ca/ca.der"))
            .expect("e2e test CA is valid DER"),
    );
    builder
}

/// Client-side deadline floor for a single blob transfer attempt, larger than
/// the default HTTP timeout because a blob can be big.
const BLOB_TRANSFER_TIMEOUT_FLOOR: std::time::Duration = std::time::Duration::from_secs(30);
/// The slowest uplink a transfer is given time to finish over, so a big blob
/// from a phone on a weak connection isn't cut off by the floor.
const SLOWEST_UPLINK_BYTES_PER_SEC: f64 = 32.0 * 1024.0;

/// The deadline for one blob transfer attempt (HTTP upload or iroh push): the
/// floor, scaled up so a big blob still fits over the slowest supported uplink.
pub fn blob_transfer_timeout(size_bytes: u64) -> std::time::Duration {
    BLOB_TRANSFER_TIMEOUT_FLOOR.max(std::time::Duration::from_secs_f64(
        size_bytes as f64 / SLOWEST_UPLINK_BYTES_PER_SEC,
    ))
}

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

    /// Announce blob hashes to the mailbox, returning the subset it reports
    /// already stored. `expect_upload` tells the mailbox to defer its fetch
    /// backstop because the bytes are about to be pushed; pass `false` when
    /// no upload is coming so the mailbox fetches immediately.
    ///
    /// The default covers in-memory/test mailboxes that do not store blobs:
    /// nothing is ever reported stored.
    async fn announce_blob_hashes(
        &self,
        _hashes: Vec<iroh_blobs::Hash>,
        _expect_upload: bool,
    ) -> Result<Vec<iroh_blobs::Hash>, anyhow::Error> {
        Ok(Vec::new())
    }

    /// Push one blob's raw bytes to the mailbox. The default errors:
    /// mailboxes that don't store blobs have nothing to upload to.
    async fn upload_blob(&self, _bytes: bytes::Bytes) -> Result<(), BlobUploadError> {
        Err(BlobUploadError::MailboxUnavailable(anyhow::anyhow!(
            "mailbox does not accept blob uploads"
        )))
    }

    /// Push one blob to the mailbox over iroh: the mailbox imports the
    /// chunk-validated stream as it arrives, so an interrupted push leaves
    /// resumable progress in its store, unlike the whole-body HTTP upload.
    /// The default errors: mailboxes without an iroh pusher have nothing to
    /// push over, and delivery falls back to `upload_blob`.
    async fn push_blob(&self, _hash: iroh_blobs::Hash) -> Result<(), BlobUploadError> {
        Err(BlobUploadError::Blob(anyhow::anyhow!(
            "mailbox does not accept iroh blob pushes"
        )))
    }
}

/// Why a single blob upload didn't succeed, so a batch can tell an
/// unreachable mailbox from a failure isolated to one blob.
#[derive(Debug, thiserror::Error)]
pub enum BlobUploadError {
    /// The mailbox itself couldn't be reached. Every other upload in the
    /// batch will hit the same wall; the caller should stop and let the
    /// announce/fetch backstop take over.
    #[error("mailbox unreachable: {0}")]
    MailboxUnavailable(anyhow::Error),
    /// The mailbox is reachable but this blob didn't store (rejection, or a
    /// timeout mid-transfer). Isolated to this blob; the batch continues.
    #[error("blob upload failed: {0}")]
    Blob(anyhow::Error),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(bound(deserialize = "Item: DeserializeOwned"))]
pub struct FetchRequest<Item: MailboxItem>(pub BTreeMap<Item::Topic, FetchTopicRequest<Item>>);

pub type FetchTopicRequest<Item> = BTreeMap<<Item as MailboxItem>::Author, SeqNum>;

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
    pub missing: HashMap<<Item as MailboxItem>::Author, Vec<SeqNum>>,
}

pub type MailboxId = String;

pub use dashchat_utils::SeqNum;

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

/// Per-mailbox record of which blob hashes a mailbox still lacks. Implemented
/// in `mailboxes-manager` over its `BlobStore`; kept as a trait here so this
/// crate stays free of manager types.
///
/// Reads and writes both live here: a sync pass asks `unfetched` for the
/// outstanding set, and the engine records what it announced and removes what
/// the mailbox confirmed. Write errors are logged by the engine (delivery
/// shouldn't fail because bookkeeping did); read errors propagate — not
/// knowing what's outstanding is unrecoverable for that pass.
#[async_trait::async_trait]
pub trait UnfetchedBlobTracker: Send + Sync + 'static {
    async fn record(
        &self,
        mailbox_id: &MailboxId,
        hashes: &[iroh_blobs::Hash],
    ) -> Result<(), anyhow::Error>;
    async fn remove(
        &self,
        mailbox_id: &MailboxId,
        hashes: &[iroh_blobs::Hash],
    ) -> Result<(), anyhow::Error>;
    /// Every hash still recorded as unfetched for this mailbox.
    async fn unfetched(
        &self,
        mailbox_id: &MailboxId,
    ) -> Result<Vec<iroh_blobs::Hash>, anyhow::Error>;
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
/// Writes are accepted and forgotten; reads report nothing outstanding.
#[derive(Clone, Default)]
pub struct NoopUnfetchedBlobTracker;

#[async_trait::async_trait]
impl UnfetchedBlobTracker for NoopUnfetchedBlobTracker {
    async fn record(
        &self,
        _mailbox_id: &MailboxId,
        _hashes: &[iroh_blobs::Hash],
    ) -> Result<(), anyhow::Error> {
        Ok(())
    }
    async fn remove(
        &self,
        _mailbox_id: &MailboxId,
        _hashes: &[iroh_blobs::Hash],
    ) -> Result<(), anyhow::Error> {
        Ok(())
    }
    async fn unfetched(
        &self,
        _mailbox_id: &MailboxId,
    ) -> Result<Vec<iroh_blobs::Hash>, anyhow::Error> {
        Ok(Vec::new())
    }
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
