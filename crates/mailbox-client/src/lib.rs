pub mod manager;
pub mod mem;
pub mod mem_server;
pub mod store;
pub mod toy;

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    sync::Arc,
    time::Duration,
};

use mailbox_api::*;
use once_cell::sync::Lazy;
use tokio::sync::{Mutex, mpsc};
use tracing::Instrument;

use serde::{Deserialize, Serialize, de::DeserializeOwned};

pub static HTTP_CLIENT: Lazy<reqwest::Client> = Lazy::new(|| {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .expect("Failed to build HTTP client")
});

pub trait MailboxClient<Item: MailboxItem>: LogStore<Item> + BlobStore {
    fn id(&self) -> MailboxId;
}

#[async_trait::async_trait]
pub trait LogStore<Item: MailboxItem>: Send + Sync + 'static {
    /// Publish items to the mailbox for the given topic.
    async fn publish(&self, ops: Vec<Item>) -> anyhow::Result<()>;

    /// Fetch items from the mailbox for the given topics.
    ///
    /// The inner map associated each author with the height of their locally stored log.
    /// The height represents the highest sequence number stored for that author, meaning that the mailbox
    /// should only return items with a higher sequence for that author.
    /// NOTE that this is a subtractive, not additive, filter, meaning that any authors not included
    /// in the `min_heights` list will have their *entire* log returned, including if `min_heights` is empty.
    /// This is so that the mailbox is used for author discovery as well.
    /// The intention is that all data is encrypted and only decipherable by valid recipients.
    async fn fetch(
        &self,
        request: FetchRequest<Item>,
    ) -> Result<FetchResponse<Item>, anyhow::Error>;
}

/// The interface for remotely storing and retrieving blobs from a mailbox server.
#[async_trait::async_trait]
pub trait BlobStore: Clone + Send + Sync + 'static {
    async fn has_blob(&self, hash: BlobHash) -> anyhow::Result<bool>;
    async fn get_blob(&self, hash: BlobHash) -> anyhow::Result<Option<Opaq>>;
    async fn store_blob(&self, blob: Opaq) -> anyhow::Result<BlobHash>;
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(bound(deserialize = "Item: DeserializeOwned"))]
pub struct FetchRequest<Item: MailboxItem>(pub BTreeMap<Item::Topic, FetchTopicRequest<Item>>);

pub type FetchTopicRequest<Item> = BTreeMap<<Item as MailboxItem>::Author, u64>;

/// Returned by the `fetch` method.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(bound(deserialize = "Item: DeserializeOwned"))]
pub struct FetchResponse<Item: MailboxItem>(pub BTreeMap<Item::Topic, FetchTopicResponse<Item>>);

/// Returned by the `fetch` method.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(bound(deserialize = "Item: DeserializeOwned"))]
pub struct FetchTopicResponse<Item: MailboxItem> {
    /// The items not held locally that were fetched.
    pub items: Vec<Item>,
    /// The items held locally that are missing from the mailbox,
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

pub trait MailboxItem: Clone + Serialize + DeserializeOwned + Send + Sync + 'static {
    type Hash: ItemTraits;
    type Author: ItemTraits;
    type Topic: ItemTraits;

    fn hash(&self) -> Self::Hash;
    fn seq_num(&self) -> SeqNum;
    fn author(&self) -> Self::Author;
    fn topic(&self) -> Self::Topic;
}

/// Extra traits for ItemTraits which are feature-dependent.
#[cfg(feature = "named-id")]
pub trait OptionalItemTraits: named_id::Rename {}
#[cfg(feature = "named-id")]
impl<T> OptionalItemTraits for T where T: named_id::Rename {}

/// Extra traits for ItemTraits which are feature-dependent.
#[cfg(not(feature = "named-id"))]
pub trait OptionalItemTraits {}
#[cfg(not(feature = "named-id"))]
impl<T> OptionalItemTraits for T {}
