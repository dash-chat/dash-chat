pub mod manager;
pub mod mem;
pub mod store;
pub mod sync_tracker;
pub mod toy;

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
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(10))
        .build()
        .expect("Failed to build HTTP client")
});

/// A mailbox server's `/health` response: its canonical MailboxId (the
/// base64url-no-pad encoding of its iroh EndpointId) and its dialing address
/// (relay + direct addresses).
#[derive(Clone, Debug, Deserialize)]
pub struct MailboxHealth {
    pub endpoint_id: MailboxId,
    pub endpoint_addr: iroh::EndpointAddr,
}

/// Fetch a mailbox server's `/health` response.
pub async fn fetch_mailbox_health(base_url: &str) -> Result<MailboxHealth, anyhow::Error> {
    let url = format!("{}/health", base_url.trim_end_matches('/'));
    Ok(HTTP_CLIENT
        .get(&url)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?)
}

#[async_trait::async_trait]
pub trait MailboxClient<Item: MailboxItem>: Send + Sync + 'static {
    fn id(&self) -> MailboxId;

    /// The base URL this client talks to, if it has one.
    fn url(&self) -> Option<String> {
        None
    }

    /// Publish an operation to the mailbox for the given topic.
    async fn publish(&self, ops: Vec<Item>) -> Result<(), anyhow::Error>;

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
    /// The operations not held locally that were fetched.
    pub items: Vec<Item>,
    /// The operations held locally that are missing from the mailbox,
    /// and which this node should now publish.
    pub missing: HashMap<<Item as MailboxItem>::Author, Vec<u64>>,
}

pub type MailboxId = String;
pub type SeqNum = u64;

pub trait ItemTraits:
    Clone + Eq + Ord + std::hash::Hash + std::fmt::Debug + Serialize + DeserializeOwned + Send + Sync
{
}

impl<T> ItemTraits for T where
    T: Clone
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

pub trait MailboxItem:
    Clone + std::fmt::Debug + Serialize + DeserializeOwned + Send + Sync + 'static
{
    type Hash: ItemTraits;
    type Author: ItemTraits;
    type Topic: ItemTraits;

    fn seq_num(&self) -> SeqNum;
    fn hash(&self) -> Self::Hash;
    fn author(&self) -> Self::Author;
    fn topic(&self) -> Self::Topic;
    fn blob_hashes(&self) -> Vec<iroh_blobs::Hash> {
        Vec::new()
    }

    /// Serialize this item into the opaque blip bytes a mailbox stores.
    /// Default: CBOR-encode the whole (self-describing) item.
    fn to_blip(&self) -> Result<mailbox_server::Blip, anyhow::Error> {
        Ok(mailbox_server::Blip::new(p2panda_core::cbor::encode_cbor(
            self,
        )?))
    }

    /// Reconstruct an item from a blip fetched from a mailbox, given the
    /// `(topic, author, seq_num)` the mailbox keyed it under. Default: CBOR-decode
    /// the blip and ignore the key parts (the item is self-describing). Override
    /// for opaque items whose topic/author/seq only exist in the key.
    fn from_blip(
        _topic: &Self::Topic,
        _author: &Self::Author,
        _seq_num: SeqNum,
        blip: &mailbox_server::Blip,
    ) -> Result<Self, anyhow::Error> {
        Ok(p2panda_core::cbor::decode_cbor(blip.as_slice())?)
    }
}

/// Extra traits for ItemTraits which are feature-dependent.
pub trait OptionalItemTraits {}
impl<T> OptionalItemTraits for T {}
