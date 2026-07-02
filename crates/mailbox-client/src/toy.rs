use std::collections::{BTreeMap, HashMap};

use mailbox_server::{Blip, GetBlipsRequest, GetBlipsResponse, StoreBlipsRequest};

use super::*;

/// Trait bounds the toy client requires of an item's `Topic` and `Author` types.
///
/// CONTRACT: the `Serialize`/`Deserialize` impls of these types MUST round-trip
/// through a single JSON string (e.g. `serializer.collect_str(&hex)`). The toy
/// client encodes topic/author ids as HTTP map keys via [`stringify`], which
/// strips the surrounding quotes; a `Serialize` impl that emits anything other
/// than a JSON string (an array, object, or number) silently produces a
/// malformed key. `dashchat-node` pins this for the real `TopicId`/`DeviceId`
/// types via its `serializes_as_json_string_for_mailbox_key` tests.
pub trait ToyItemTraits: ItemTraits + Serialize + DeserializeOwned {}
impl<T> ToyItemTraits for T where T: ItemTraits + Serialize + DeserializeOwned {}

/// A client for the toy mailbox server.
#[derive(Clone)]
pub struct ToyMailboxClient<Item: MailboxItem> {
    id: MailboxId,
    base_url: String,
    sender_pubkey: iroh::EndpointId,
    phantom: std::marker::PhantomData<Item>,
}

impl<Item: MailboxItem> ToyMailboxClient<Item> {
    pub fn new(
        id: MailboxId,
        base_url: impl Into<String>,
        sender_pubkey: iroh::EndpointId,
    ) -> Self {
        Self {
            id,
            base_url: base_url.into(),
            sender_pubkey,
            phantom: std::marker::PhantomData,
        }
    }
}

#[async_trait::async_trait]
impl<Item: MailboxItem> MailboxClient<Item> for ToyMailboxClient<Item>
where
    Item::Topic: ToyItemTraits,
    Item::Author: ToyItemTraits,
{
    fn id(&self) -> MailboxId {
        self.id.clone()
    }

    fn url(&self) -> Option<String> {
        Some(self.base_url.clone())
    }

    async fn publish(&self, ops: Vec<Item>) -> Result<(), anyhow::Error> {
        if ops.is_empty() {
            return Ok(());
        }

        // Group operations by topic -> author -> seq_num
        let mut blips: BTreeMap<String, BTreeMap<String, BTreeMap<u64, Blip>>> = BTreeMap::new();

        for op in ops {
            let topic_id = Self::encode_topic_id(&op.topic());
            let log_id = Self::device_id_to_log_id(&op.author());
            let seq_num = op.seq_num();
            let blip = Self::serialize_operation(&op)?;

            blips
                .entry(topic_id)
                .or_default()
                .entry(log_id)
                .or_default()
                .insert(seq_num, blip);
        }

        let request = StoreBlipsRequest {
            blips,
            sender_pubkey: Some(self.sender_pubkey),
            signature: Vec::new(),
        };
        let response = HTTP_CLIENT
            .post(format!("{}/blips/store", self.base_url))
            .json(&request)
            .send()
            .await?;

        if response.status().is_success() {
            Ok(())
        } else {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            Err(anyhow::anyhow!(
                "Failed to store blips: {} - {}",
                status,
                body
            ))
        }
    }

    async fn fetch(
        &self,
        request: FetchRequest<Item>,
    ) -> Result<FetchResponse<Item>, anyhow::Error> {
        // Convert FetchRequest to GetBlipsRequest
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

        let get_request = GetBlipsRequest { topics };
        let response = HTTP_CLIENT
            .post(format!("{}/blips/get", self.base_url))
            .json(&get_request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Failed to fetch blips: {} - {}",
                status,
                body
            ));
        }

        let response = response.json::<GetBlipsResponse>().await?;

        // Convert GetBlipsResponse to FetchResponse
        let mut result: BTreeMap<Item::Topic, FetchTopicResponse<Item>> = BTreeMap::new();

        for (topic_id_str, topic_response) in response.blips_by_topic {
            let log_id = Self::log_id_from_string(&topic_id_str)?;

            // Deserialize blips to operations
            let mut items = Vec::new();
            for (_author_str, seq_blips) in topic_response.blips {
                for (_seq, blip) in seq_blips {
                    items.push(Self::deserialize_operation(&blip)?);
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
    fn encode_topic_id(topic_id: &Item::Topic) -> String {
        stringify(topic_id)
    }

    fn device_id_to_log_id(device_id: &Item::Author) -> String {
        stringify(device_id)
    }

    fn log_id_from_string(s: &str) -> Result<Item::Topic, anyhow::Error> {
        let topic: Item::Topic = unstringify(s)?;
        Ok(topic)
    }

    fn device_id_from_string(s: &str) -> Result<Item::Author, anyhow::Error> {
        let author: Item::Author = unstringify(s)?;
        Ok(author)
    }

    fn serialize_operation(item: &Item) -> Result<Blip, anyhow::Error> {
        let bytes = p2panda_core::cbor::encode_cbor(item)?;
        Ok(Blip::new(bytes))
    }

    fn deserialize_operation(blip: &Blip) -> Result<Item, anyhow::Error> {
        Ok(p2panda_core::cbor::decode_cbor(blip.as_slice())?)
    }
}

pub fn stringify(value: impl Serialize) -> String {
    serde_json::to_string(&value)
        .expect("value is JSON-serializable")
        .trim_matches('"')
        .to_string()
}

pub fn unstringify<T: DeserializeOwned>(s: &str) -> Result<T, anyhow::Error> {
    serde_json::from_str(&format!("\"{}\"", s))
        .map_err(|e| anyhow::anyhow!("Failed to unstringify: {}", e))
}

/// Poll the mailbox `/health` endpoint until it responds, confirming the server
/// is listening before clients try to use it.
pub async fn wait_for_mailbox_health(url: &str) {
    let health = format!("{url}/health");
    for _ in 0..100 {
        if let Ok(resp) = crate::HTTP_CLIENT.get(&health).send().await {
            if resp.status().is_success() {
                return;
            }
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }
    panic!("mailbox /health never became ready at {health}");
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::*;

    #[derive(Debug, PartialEq)]
    struct Abecedarian(u8);

    impl Serialize for Abecedarian {
        fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            serializer.serialize_str(&format!(
                "{}",
                "abcdefghijklmnopqrstuvwxyz"
                    .chars()
                    .take(self.0 as usize)
                    .collect::<String>()
            ))
        }
    }

    impl<'de> Deserialize<'de> for Abecedarian {
        fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            let s = String::deserialize(deserializer)?;
            let value = s.chars().count() as u8;
            Ok(Abecedarian(value))
        }
    }

    #[test]
    fn test_stringify_unstringify() {
        let topic = Abecedarian(10);
        let topic_str = stringify(&topic);
        assert_eq!(topic_str, "abcdefghij");
        let topic_unstr = unstringify(&topic_str).unwrap();
        assert_eq!(topic, topic_unstr);
    }
}
