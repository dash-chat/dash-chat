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

/// Client-side timeout for a single blob upload, larger than the default HTTP
/// timeout because a blob can be big.
const UPLOAD_BLOB_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

/// POST blob hashes to a mailbox's `/blobs/store`, returning the subset the
/// mailbox reports it already has stored.
pub async fn send_store_blobs(
    base_url: &str,
    hashes: Vec<iroh_blobs::Hash>,
    sender_pubkey: iroh::EndpointId,
) -> anyhow::Result<Vec<iroh_blobs::Hash>> {
    if hashes.is_empty() {
        return Ok(Vec::new());
    }
    let request = mailbox_server::StoreBlobsRequest {
        blob_hashes: hashes,
        sender_pubkey,
        signature: Vec::new(),
    };
    let response = HTTP_CLIENT
        .post(format!("{base_url}/blobs/store"))
        .json(&request)
        .send()
        .await?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("Failed to store blobs: {status} - {body}");
    }
    let response: mailbox_server::StoreBlobsResponse = response.json().await?;
    Ok(response.already_stored)
}

/// POST a single blob's raw bytes to a mailbox's `/blobs/upload`. The body is the
/// bytes themselves — no JSON/base64 wrapping — so the on-wire size matches the
/// blob.
async fn upload_blob(base_url: &str, bytes: bytes::Bytes) -> anyhow::Result<()> {
    let response = HTTP_CLIENT
        .post(format!("{base_url}/blobs/upload"))
        .timeout(UPLOAD_BLOB_TIMEOUT)
        .body(bytes)
        .send()
        .await?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("Failed to upload blob: {status} - {body}");
    }
    Ok(())
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

    /// Attach a blob-bytes source so `publish` pushes blob bytes straight to the
    /// mailbox (best-effort) before announcing their hashes. Without a reader the
    /// client only announces hashes and the mailbox fetches the bytes from us.
    pub fn with_blob_reader(
        mut self,
        blob_reader: std::sync::Arc<dyn crate::BlobReader>,
    ) -> Self {
        self.blob_reader = Some(blob_reader);
        self
    }

    /// Push blob bytes to the mailbox immediately (best-effort), then announce
    /// their hashes and reconcile the unfetched tracker: record hashes the
    /// mailbox still needs, remove any it already has. The announce always runs,
    /// so any blob that failed to upload is still fetched by the mailbox and
    /// retried by the followup task.
    async fn store_blobs(&self, hashes: Vec<iroh_blobs::Hash>) -> anyhow::Result<()> {
        if hashes.is_empty() {
            return Ok(());
        }
        self.upload_blobs(&hashes).await;
        let already_stored =
            send_store_blobs(&self.base_url, hashes.clone(), self.sender_pubkey).await?;
        let not_stored: Vec<_> = hashes
            .into_iter()
            .filter(|h| !already_stored.contains(h))
            .collect();
        self.tracker.record(&self.id, &not_stored).await;
        self.tracker.remove(&self.id, &already_stored).await;
        Ok(())
    }

    /// Best-effort: read each blob from the configured reader and upload its
    /// bytes to the mailbox, one at a time (so at most one blob is held in
    /// memory). Logs and continues past any read or upload error — the caller's
    /// announce is the source of truth, so a missed upload just means the mailbox
    /// fetches that blob instead. No-op when no reader is configured.
    async fn upload_blobs(&self, hashes: &[iroh_blobs::Hash]) {
        let Some(reader) = self.blob_reader.as_ref() else {
            return;
        };
        for hash in hashes {
            let bytes = match reader.read_blob(*hash).await {
                Ok(bytes) => bytes,
                Err(err) => {
                    tracing::warn!(%hash, ?err, "failed to read blob for upload; relying on announce");
                    continue;
                }
            };
            if let Err(err) = upload_blob(&self.base_url, bytes).await {
                tracing::warn!(%hash, ?err, "blob upload failed; relying on announce");
            }
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

        let _tracker = std::sync::Arc::new(RecordingTracker::default());
        let already = crate::toy::send_store_blobs(
            &base_url,
            vec![h_stored, h_new],
            iroh::SecretKey::from_bytes(&[3; 32]).public(),
        )
        .await
        .unwrap();
        assert_eq!(already, vec![h_stored]);
    }

    #[tokio::test]
    async fn upload_blob_posts_raw_bytes() {
        use std::sync::{Arc, Mutex};

        // Server that records the raw body it received and echoes back its hash.
        let received: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
        let received_in_handler = received.clone();
        let app = axum::Router::new().route(
            "/blobs/upload",
            axum::routing::post(move |body: axum::body::Bytes| {
                let received_in_handler = received_in_handler.clone();
                async move {
                    let hash = iroh_blobs::Hash::new(&body);
                    *received_in_handler.lock().unwrap() = body.to_vec();
                    axum::Json(mailbox_server::UploadBlobResponse { hash })
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let base_url = format!("http://{addr}");

        let data = bytes::Bytes::from_static(b"raw blob bytes");
        crate::toy::upload_blob(&base_url, data.clone())
            .await
            .unwrap();
        assert_eq!(*received.lock().unwrap(), data.to_vec());
    }

    #[tokio::test]
    async fn store_blobs_uploads_bytes_then_announces() {
        use std::sync::{Arc, Mutex};

        struct StubReader(bytes::Bytes);
        #[async_trait::async_trait]
        impl crate::BlobReader for StubReader {
            async fn read_blob(&self, _hash: iroh_blobs::Hash) -> anyhow::Result<bytes::Bytes> {
                Ok(self.0.clone())
            }
        }

        let data = bytes::Bytes::from_static(b"a blob");
        let hash = iroh_blobs::Hash::new(&data);

        // `/blobs/upload` records the pushed bytes; once uploaded, `/blobs/store`
        // reports the announced hash as already stored.
        let uploaded: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
        let uploaded_in_handler = uploaded.clone();
        let uploaded_for_store = uploaded.clone();
        let app = axum::Router::new()
            .route(
                "/blobs/upload",
                axum::routing::post(move |body: axum::body::Bytes| {
                    let uploaded_in_handler = uploaded_in_handler.clone();
                    async move {
                        let hash = iroh_blobs::Hash::new(&body);
                        *uploaded_in_handler.lock().unwrap() = body.to_vec();
                        axum::Json(mailbox_server::UploadBlobResponse { hash })
                    }
                }),
            )
            .route(
                "/blobs/store",
                axum::routing::post(
                    move |axum::Json(req): axum::Json<mailbox_server::StoreBlobsRequest>| async move {
                        let has_upload = !uploaded_for_store.lock().unwrap().is_empty();
                        let already_stored = if has_upload {
                            req.blob_hashes
                        } else {
                            Vec::new()
                        };
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

        let client = ToyMailboxClient::<crate::testing::Msg>::new(
            "mbx".to_string(),
            base_url,
            iroh::SecretKey::from_bytes(&[3; 32]).public(),
            std::sync::Arc::new(crate::NoopUnfetchedBlobTracker),
        )
        .with_blob_reader(std::sync::Arc::new(StubReader(data.clone())));

        client.store_blobs(vec![hash]).await.unwrap();
        assert_eq!(*uploaded.lock().unwrap(), data.to_vec());
    }
}
