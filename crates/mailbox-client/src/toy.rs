use std::collections::{BTreeMap, HashMap};

use mailbox_api::{GetDollopsRequest, GetDollopsResponse, Opaq, StoreDollopsRequest};

use super::*;

pub trait ToyItemTraits: ItemTraits {
    fn as_bytes(&self) -> &[u8];
    fn from_str(s: &str) -> Result<Self, anyhow::Error>;
}

/// A client for the toy mailbox server.
#[derive(Clone)]
pub struct ToyMailboxClient<Item: MailboxItem> {
    id: MailboxId,
    base_url: String,
    phantom: std::marker::PhantomData<Item>,
}

impl<Item: MailboxItem> ToyMailboxClient<Item> {
    pub fn new(id: MailboxId, base_url: impl Into<String>) -> Self {
        Self {
            id,
            base_url: base_url.into(),
            phantom: std::marker::PhantomData,
        }
    }
}

impl<Item: MailboxItem> MailboxClient<Item> for ToyMailboxClient<Item>
where
    Item::Topic: ToyItemTraits,
    Item::Author: ToyItemTraits,
{
    fn id(&self) -> MailboxId {
        self.id.clone()
    }
}

#[async_trait::async_trait]
impl<Item: MailboxItem> BlobStore for ToyMailboxClient<Item>
where
    Item::Topic: ToyItemTraits,
    Item::Author: ToyItemTraits,
{
    async fn has_blob(&self, hash: BlobHash) -> anyhow::Result<bool> {
        todo!()
    }

    async fn get_blob(&self, hash: BlobHash) -> anyhow::Result<Option<Opaq>> {
        todo!()
    }

    async fn store_blob(&self, blob: Opaq) -> anyhow::Result<BlobHash> {
        todo!()
    }
}

#[async_trait::async_trait]
impl<Item: MailboxItem> LogStore<Item> for ToyMailboxClient<Item>
where
    Item::Topic: ToyItemTraits,
    Item::Author: ToyItemTraits,
{
    async fn publish(&self, ops: Vec<Item>) -> Result<(), anyhow::Error> {
        if ops.is_empty() {
            return Ok(());
        }

        // Group items by topic -> author -> seq_num
        let mut dollops: BTreeMap<String, BTreeMap<String, BTreeMap<u64, Opaq>>> = BTreeMap::new();

        for op in ops {
            let topic_id = Self::encode_topic_id(&op.topic());
            let log_id = Self::device_id_to_log_id(&op.author());
            let seq_num = op.seq_num();
            let blob = Self::serialize_operation(&op)?;

            dollops
                .entry(topic_id)
                .or_default()
                .entry(log_id)
                .or_default()
                .insert(seq_num, blob);
        }

        let request = StoreDollopsRequest { dollops };
        let response = HTTP_CLIENT
            .post(format!("{}/dollops/store", self.base_url))
            .json(&request)
            .send()
            .await?;

        if response.status().is_success() {
            Ok(())
        } else {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            Err(anyhow::anyhow!(
                "Failed to store dollops: {} - {}",
                status,
                body
            ))
        }
    }

    async fn fetch(
        &self,
        request: FetchRequest<Item>,
    ) -> Result<FetchResponse<Item>, anyhow::Error> {
        // Convert FetchRequest to GetdollopsRequest
        let mut topics: BTreeMap<String, BTreeMap<String, u64>> = BTreeMap::new();

        for (log_id, authors) in request.0.iter() {
            let topic_id = Self::encode_topic_id(log_id);
            let mut log_map: BTreeMap<String, u64> = BTreeMap::new();

            for (device_id, height) in authors.iter() {
                let server_log_id = Self::device_id_to_log_id(device_id);
                log_map.insert(server_log_id, *height);
            }

            topics.insert(topic_id, log_map);
        }

        let get_request = GetDollopsRequest { topics };
        let response = HTTP_CLIENT
            .post(format!("{}/dollops/get", self.base_url))
            .json(&get_request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Failed to fetch dollops: {} - {}",
                status,
                body
            ));
        }

        let response = response.json::<GetDollopsResponse>().await?;

        // Convert GetDollopsResponse to FetchResponse
        let mut result: BTreeMap<Item::Topic, FetchTopicResponse<Item>> = BTreeMap::new();

        for (topic_id_str, topic_response) in response.dollops_by_topic {
            let log_id = Self::log_id_from_string(&topic_id_str)?;

            // Deserialize dollops to items
            let mut items = Vec::new();
            for (_author_str, seq_dollops) in topic_response.dollops {
                for (_seq, blob) in seq_dollops {
                    items.push(Self::deserialize_operation(&blob)?);
                }
            }

            // Convert missing map
            let mut missing: HashMap<Item::Author, Vec<u64>> = HashMap::new();
            for (author_str, seq_nums) in topic_response.missing {
                let device_id = Self::device_id_from_string(&author_str)?;
                missing.insert(device_id, seq_nums);
            }

            result.insert(log_id, FetchTopicResponse { items, missing });
        }

        Ok(FetchResponse(result))
    }
}

impl<Item: MailboxItem> ToyMailboxClient<Item>
where
    Item::Topic: ToyItemTraits,
    Item::Author: ToyItemTraits,
{
    /// Helper functions

    fn encode_topic_id(topic_id: &Item::Topic) -> String {
        hex::encode(topic_id.as_bytes())
    }

    fn device_id_to_log_id(device_id: &Item::Author) -> String {
        hex::encode(device_id.as_bytes())
    }

    fn log_id_from_string(s: &str) -> Result<Item::Topic, anyhow::Error> {
        let topic: Item::Topic = Item::Topic::from_str(s)?;
        Ok(topic)
    }

    fn device_id_from_string(s: &str) -> Result<Item::Author, anyhow::Error> {
        let author: Item::Author = Item::Author::from_str(s)?;
        Ok(author)
    }

    fn serialize_operation(item: &Item) -> Result<Opaq, anyhow::Error> {
        let bytes = p2panda_core::cbor::encode_cbor(item)?;
        Ok(Opaq::new(bytes))
    }

    fn deserialize_operation(blob: &Opaq) -> Result<Item, anyhow::Error> {
        Ok(p2panda_core::cbor::decode_cbor(blob.as_ref())?)
    }
}
