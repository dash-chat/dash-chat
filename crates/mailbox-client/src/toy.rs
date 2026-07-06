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

/// Client-side timeout for a `/blobs/store` request that carries blob bytes
/// inline; larger than the default HTTP timeout because the upload can be big.
const STORE_BLOBS_WITH_BYTES_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

/// POST blob hashes (and optionally the blob bytes themselves) to a mailbox's
/// `/blobs/store`, returning the subset the mailbox reports it already has
/// stored. When `blobs` is `Some`, a longer per-request timeout is applied.
pub async fn send_store_blobs(
    base_url: &str,
    blobs: Option<Vec<bytes::Bytes>>,
    hashes: Vec<iroh_blobs::Hash>,
    sender_pubkey: iroh::EndpointId,
) -> anyhow::Result<Vec<iroh_blobs::Hash>> {
    if hashes.is_empty() {
        return Ok(Vec::new());
    }
    let has_blobs = blobs.is_some();
    let request = mailbox_server::StoreBlobsRequest {
        blobs,
        blob_hashes: hashes,
        sender_pubkey,
        signature: Vec::new(),
    };
    let mut builder = HTTP_CLIENT
        .post(format!("{base_url}/blobs/store"))
        .json(&request);
    if has_blobs {
        builder = builder.timeout(STORE_BLOBS_WITH_BYTES_TIMEOUT);
    }
    let response = builder.send().await?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("Failed to store blobs: {status} - {body}");
    }
    let response: mailbox_server::StoreBlobsResponse = response.json().await?;
    Ok(response.already_stored)
}

/// A client for the toy mailbox server.
#[derive(Clone)]
pub struct ToyMailboxClient<Item: MailboxItem> {
    id: MailboxId,
    base_url: String,
    sender_pubkey: iroh::EndpointId,
    tracker: std::sync::Arc<dyn crate::UnfetchedBlobTracker>,
    blob_reader: Option<std::sync::Arc<dyn crate::BlobReader>>,
    phantom: std::marker::PhantomData<Item>,
}

impl<Item: MailboxItem> ToyMailboxClient<Item> {
    pub fn new(
        id: MailboxId,
        base_url: impl Into<String>,
        sender_pubkey: iroh::EndpointId,
        tracker: std::sync::Arc<dyn crate::UnfetchedBlobTracker>,
    ) -> Self {
        Self {
            id,
            base_url: base_url.into(),
            sender_pubkey,
            tracker,
            blob_reader: None,
            phantom: std::marker::PhantomData,
        }
    }

    /// Attach a blob-bytes source so `publish` uploads blob bytes inline to the
    /// mailbox (falling back to announcing hashes only on timeout). Without a
    /// reader the client only announces hashes and the mailbox fetches them.
    pub fn with_blob_reader(
        mut self,
        blob_reader: std::sync::Arc<dyn crate::BlobReader>,
    ) -> Self {
        self.blob_reader = Some(blob_reader);
        self
    }

    /// Announce blob hashes to this mailbox and reconcile the unfetched tracker:
    /// record hashes the mailbox still needs, remove any it already has.
    async fn store_blobs(&self, hashes: Vec<iroh_blobs::Hash>) -> anyhow::Result<()> {
        if hashes.is_empty() {
            return Ok(());
        }
        let already_stored = self.upload_blobs(hashes.clone()).await?;
        let not_stored: Vec<_> = hashes
            .into_iter()
            .filter(|h| !already_stored.contains(h))
            .collect();
        self.tracker.record(&self.id, &not_stored).await;
        self.tracker.remove(&self.id, &already_stored).await;
        Ok(())
    }

    /// Upload blob bytes inline (with a 15s timeout) when a blob reader is
    /// available; if that upload times out — or no reader is configured — send
    /// the hashes alone so the mailbox fetches the blobs from us instead.
    async fn upload_blobs(
        &self,
        hashes: Vec<iroh_blobs::Hash>,
    ) -> anyhow::Result<Vec<iroh_blobs::Hash>> {
        if let Some(blobs) = self.read_blobs(&hashes).await {
            match send_store_blobs(&self.base_url, Some(blobs), hashes.clone(), self.sender_pubkey)
                .await
            {
                Ok(already_stored) => return Ok(already_stored),
                Err(err) if is_timeout(&err) => {
                    tracing::warn!(
                        "store_blobs with inline bytes timed out; retrying with hashes only"
                    );
                }
                Err(err) => return Err(err),
            }
        }
        send_store_blobs(&self.base_url, None, hashes, self.sender_pubkey).await
    }

    /// Read every blob's bytes from the configured reader, returning `None` (so
    /// the caller announces hashes only) when there is no reader or any read
    /// fails.
    async fn read_blobs(&self, hashes: &[iroh_blobs::Hash]) -> Option<Vec<bytes::Bytes>> {
        let reader = self.blob_reader.as_ref()?;
        let mut blobs = Vec::with_capacity(hashes.len());
        for hash in hashes {
            match reader.read_blob(*hash).await {
                Ok(bytes) => blobs.push(bytes),
                Err(err) => {
                    tracing::warn!(%hash, ?err, "failed to read blob bytes; announcing hashes only");
                    return None;
                }
            }
        }
        Some(blobs)
    }
}

/// True when the error was caused by a request timeout, so the caller can retry
/// via a different path rather than surfacing the failure.
fn is_timeout(err: &anyhow::Error) -> bool {
    err.downcast_ref::<reqwest::Error>()
        .is_some_and(|e| e.is_timeout())
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

        let blob_hashes: Vec<iroh_blobs::Hash> =
            ops.iter().flat_map(|op| op.blob_hashes()).collect();

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
            self.store_blobs(blob_hashes).await?;
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

    #[tokio::test]
    async fn store_blobs_records_not_stored_and_removes_already_stored() {
        use std::sync::Mutex as StdMutex;

        #[derive(Default)]
        struct RecordingTracker {
            recorded: StdMutex<Vec<(String, Vec<iroh_blobs::Hash>)>>,
            removed: StdMutex<Vec<(String, Vec<iroh_blobs::Hash>)>>,
        }
        #[async_trait::async_trait]
        impl crate::UnfetchedBlobTracker for RecordingTracker {
            async fn record(&self, id: &crate::MailboxId, hashes: &[iroh_blobs::Hash]) {
                self.recorded
                    .lock()
                    .unwrap()
                    .push((id.clone(), hashes.to_vec()));
            }
            async fn remove(&self, id: &crate::MailboxId, hashes: &[iroh_blobs::Hash]) {
                self.removed
                    .lock()
                    .unwrap()
                    .push((id.clone(), hashes.to_vec()));
            }
        }

        // Server that reports h_stored as already stored, h_new as not.
        let h_stored = iroh_blobs::Hash::new([1; 32]);
        let h_new = iroh_blobs::Hash::new([2; 32]);
        let app = axum::Router::new().route(
            "/blobs/store",
            axum::routing::post(
                move |axum::Json(req): axum::Json<mailbox_server::StoreBlobsRequest>| async move {
                    let already_stored: Vec<_> = req
                        .blob_hashes
                        .into_iter()
                        .filter(|h| *h == h_stored)
                        .collect();
                    axum::Json(mailbox_server::StoreBlobsResponse { already_stored })
                },
            ),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let base_url = format!("http://{addr}");

        let tracker = std::sync::Arc::new(RecordingTracker::default());
        let already = crate::toy::send_store_blobs(
            &base_url,
            None,
            vec![h_stored, h_new],
            iroh::SecretKey::from_bytes(&[3; 32]).public(),
        )
        .await
        .unwrap();
        assert_eq!(already, vec![h_stored]);
    }

    #[tokio::test]
    async fn store_blobs_sends_inline_bytes_and_reports_them_stored() {
        // Server that hashes each inline blob it receives and reports those
        // hashes as already stored (mirroring the real mailbox behavior).
        let app = axum::Router::new().route(
            "/blobs/store",
            axum::routing::post(
                |axum::Json(req): axum::Json<mailbox_server::StoreBlobsRequest>| async move {
                    let stored: Vec<_> = req
                        .blobs
                        .unwrap_or_default()
                        .iter()
                        .map(iroh_blobs::Hash::new)
                        .filter(|h| req.blob_hashes.contains(h))
                        .collect();
                    axum::Json(mailbox_server::StoreBlobsResponse {
                        already_stored: stored,
                    })
                },
            ),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let base_url = format!("http://{addr}");

        let data = bytes::Bytes::from_static(b"inline blob");
        let hash = iroh_blobs::Hash::new(&data);
        let already = crate::toy::send_store_blobs(
            &base_url,
            Some(vec![data]),
            vec![hash],
            iroh::SecretKey::from_bytes(&[3; 32]).public(),
        )
        .await
        .unwrap();
        assert_eq!(already, vec![hash]);
    }
}
