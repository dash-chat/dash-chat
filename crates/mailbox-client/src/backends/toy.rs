use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use mailbox_server::{
    Blip, GetBlipsRequest, GetBlipsResponse, StoreBlipsRequest, StoreBlipsResponse,
};

use crate::{
    BlobUploadLifecycle, FetchRequest, FetchResponse, FetchTopicResponse, HTTP_CLIENT,
    MailboxClient, MailboxId, MailboxItem, MailboxKey, PublishResponse,
};

/// Client-side timeout for a single blob upload, larger than the default HTTP
/// timeout because a blob can be big.
const UPLOAD_BLOB_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);
/// The slowest uplink an upload is given time to finish over, so a big blob
/// from a phone on a weak connection isn't cut off by [`UPLOAD_BLOB_TIMEOUT`].
const SLOWEST_UPLINK_BYTES_PER_SEC: f64 = 32.0 * 1024.0;

/// Why a single blob upload attempt didn't succeed, so the caller can tell an
/// unreachable mailbox from a failure isolated to one blob.
#[derive(Debug)]
enum UploadError {
    /// The mailbox itself couldn't be reached (connection refused/reset). Every
    /// other upload in the batch will hit the same wall, so the caller stops and
    /// lets the announce/fetch backstop take over.
    MailboxUnavailable(anyhow::Error),
    /// The mailbox is reachable but this blob didn't store (non-success status,
    /// or the request timed out mid-transfer). Isolated to this blob, so the
    /// caller skips it and keeps uploading the rest.
    Blob(anyhow::Error),
}

/// A connection-level failure means the mailbox is unreachable; anything else
/// (including a per-request timeout on one large blob) is scoped to that blob so
/// a single slow or rejected upload doesn't abort the whole batch.
fn classify_upload_error(err: reqwest::Error) -> UploadError {
    if err.is_connect() {
        UploadError::MailboxUnavailable(err.into())
    } else {
        UploadError::Blob(err.into())
    }
}

/// POST blob hashes to a mailbox's `/blobs/register-hashes`, returning the subset the
/// mailbox reports it already has stored. Set `expect_upload` when the caller
/// will stream the bytes to `/blobs/upload` right after, so the mailbox defers
/// its fetch backstop by its own fixed grace window and lets that upload land
/// first without a duplicate transfer; pass `false` to have the mailbox fetch
/// immediately (no upload is coming).
async fn send_register_hashes(
    base_url: &str,
    hashes: Vec<iroh_blobs::Hash>,
    sender_pubkey: iroh::EndpointId,
    expect_upload: bool,
) -> anyhow::Result<Vec<iroh_blobs::Hash>> {
    if hashes.is_empty() {
        return Ok(Vec::new());
    }
    let request = mailbox_server::RegisterHashesRequest {
        blob_hashes: hashes,
        sender_pubkey,
        expect_upload,
        signature: Vec::new(),
    };
    let response = HTTP_CLIENT
        .post(format!("{base_url}/blobs/register-hashes"))
        .json(&request)
        .send()
        .await?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("Failed to store blobs: {status} - {body}");
    }
    let response: mailbox_server::RegisterHashesResponse = response.json().await?;
    Ok(response.already_stored)
}

/// POST a single blob's raw bytes to a mailbox's `/blobs/upload`. The body is the
/// bytes themselves — no JSON/base64 wrapping — so the on-wire size matches the
/// blob.
async fn upload_blob(base_url: &str, bytes: bytes::Bytes) -> Result<(), UploadError> {
    let response = HTTP_CLIENT
        .post(format!("{base_url}/blobs/upload"))
        .timeout(UPLOAD_BLOB_TIMEOUT.max(std::time::Duration::from_secs_f64(
            bytes.len() as f64 / SLOWEST_UPLINK_BYTES_PER_SEC,
        )))
        .body(bytes)
        .send()
        .await
        .map_err(classify_upload_error)?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(UploadError::Blob(anyhow::anyhow!(
            "Failed to upload blob: {status} - {body}"
        )));
    }
    Ok(())
}

/// Stand-in lifecycle used when a `ToyMailboxClient` is exercised before being
/// registered with a `Mailboxes` owner. Every claim succeeds and finishes are
/// ignored, so uploads proceed without coordination.
struct NoopUploadLifecycle;

impl BlobUploadLifecycle for NoopUploadLifecycle {
    fn claim_upload(&self, base_url: &str, hash: iroh_blobs::Hash) -> Option<u64> {
        tracing::warn!(
            %base_url,
            %hash,
            "blob upload lifecycle not configured; upload will not be coordinated"
        );
        Some(0)
    }

    fn finish_upload(
        &self,
        base_url: &str,
        hash: iroh_blobs::Hash,
        _claim: u64,
        _succeeded: bool,
    ) {
        tracing::warn!(
            %base_url,
            %hash,
            "blob upload lifecycle not configured; finish ignored"
        );
    }
}

/// A client for the toy mailbox server.
#[derive(Clone)]
pub struct ToyMailboxClient<Item: MailboxItem> {
    id: MailboxId,
    base_url: String,
    sender_pubkey: iroh::EndpointId,
    tracker: std::sync::Arc<dyn crate::UnfetchedBlobTracker>,
    blob_reader: Option<std::sync::Arc<dyn crate::BlobReader>>,
    lifecycle: std::sync::Arc<dyn BlobUploadLifecycle>,
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
            lifecycle: std::sync::Arc::new(NoopUploadLifecycle),
            phantom: std::marker::PhantomData,
        }
    }

    /// Attach a blob-bytes source so `publish` streams blob bytes to the mailbox
    /// (best-effort, in a spawned task) after announcing their hashes. Without a
    /// reader the client only announces hashes and the mailbox fetches the bytes
    /// from us.
    pub fn with_blob_reader(mut self, blob_reader: std::sync::Arc<dyn crate::BlobReader>) -> Self {
        self.blob_reader = Some(blob_reader);
        self
    }

    /// Attach the shared upload lifecycle; called by `Mailboxes::register`.
    pub fn set_upload_lifecycle(
        &mut self,
        lifecycle: std::sync::Arc<dyn BlobUploadLifecycle>,
    ) {
        self.lifecycle = lifecycle;
    }

    /// Announce blob hashes to the mailbox and reconcile the unfetched tracker,
    /// then push the bytes the mailbox still needs in a detached best-effort task.
    ///
    /// The announce goes first and is awaited: it is what atomically locks in the
    /// publish (the mailbox now knows to fetch these hashes as a backstop). Only
    /// then do we stream the bytes the mailbox reported it lacks — spawned, so a
    /// batch of large blobs never stalls this per-mailbox publish iteration, and
    /// scoped to `not_stored`, so we never re-upload blobs the mailbox already
    /// holds.
    pub async fn store_blobs(&self, hashes: Vec<iroh_blobs::Hash>) -> anyhow::Result<()> {
        self.store_blobs_with(
            hashes,
            self.blob_reader.clone(),
            self.tracker.clone(),
            self.lifecycle.clone(),
        )
        .await
    }

    async fn store_blobs_with(
        &self,
        mut hashes: Vec<iroh_blobs::Hash>,
        reader: Option<Arc<dyn crate::BlobReader>>,
        tracker: Arc<dyn crate::UnfetchedBlobTracker>,
        lifecycle: Arc<dyn crate::BlobUploadLifecycle>,
    ) -> anyhow::Result<()> {
        if hashes.is_empty() {
            return Ok(());
        }
        // A forwarded op's blob this device hasn't fetched is left out: the
        // mailbox would dial us for bytes we don't have.
        if let Some(reader) = &reader {
            let mut held = Vec::new();
            for hash in hashes {
                if reader.has_blob(hash).await {
                    held.push(hash);
                }
            }
            hashes = held;
        }
        // Tell the mailbox to defer its fetch backstop only when we can actually
        // stream the bytes; a reader-less client never uploads, so the mailbox
        // should fetch from us right away.
        let expect_upload = reader.is_some();
        let already_stored = send_register_hashes(
            &self.base_url,
            hashes.clone(),
            self.sender_pubkey,
            expect_upload,
        )
        .await?;
        let not_stored: Vec<_> = hashes
            .into_iter()
            .filter(|h| !already_stored.contains(h))
            .collect();
        tracker.record(&self.id, &not_stored).await;
        tracker.remove(&self.id, &already_stored).await;
        self.spawn_blob_upload_with(reader, tracker, lifecycle, not_stored);
        Ok(())
    }

    /// Spawn a detached best-effort task that streams each blob's bytes to the
    /// mailbox, one at a time (so at most one blob is held in memory). Every blob
    /// that uploads successfully is removed from the unfetched tracker (the
    /// mailbox now holds it). We keep going through the batch as long as uploads
    /// are feasible: a blob we can't read or that the mailbox rejects is left in
    /// the tracker and skipped, but the moment the mailbox itself is unreachable
    /// we stop — the remaining blobs stay queued for the mailbox's fetch backstop
    /// rather than burning through the batch against a dead endpoint. No-op when
    /// no blob reader is configured.
    fn spawn_blob_upload_with(
        &self,
        reader: Option<Arc<dyn crate::BlobReader>>,
        tracker: Arc<dyn crate::UnfetchedBlobTracker>,
        lifecycle: Arc<dyn crate::BlobUploadLifecycle>,
        hashes: Vec<iroh_blobs::Hash>,
    ) {
        let Some(reader) = reader else {
            return;
        };
        // Claimed up front, so a later followup pass doesn't start the ones this
        // task hasn't reached yet alongside it over the same uplink.
        let claimed: Vec<(iroh_blobs::Hash, u64)> = hashes
            .into_iter()
            .filter_map(|hash| Some((hash, lifecycle.claim_upload(&self.base_url, hash)?)))
            .collect();
        if claimed.is_empty() {
            return;
        }
        let base_url = self.base_url.clone();
        let id = self.id.clone();
        tokio::spawn(async move {
            let mut claimed = claimed.into_iter();
            while let Some((hash, claim)) = claimed.next() {
                let bytes = match reader.read_blob(hash).await {
                    Ok(bytes) => bytes,
                    Err(err) => {
                        tracing::warn!(%hash, ?err, "failed to read blob for upload; relying on announce");
                        lifecycle.finish_upload(&base_url, hash, claim, false);
                        continue;
                    }
                };
                let size = bytes.len();
                tracing::info!(%hash, size, "uploading blob");
                match upload_blob(&base_url, bytes).await {
                    Ok(()) => {
                        tracing::info!(%hash, size, "uploaded blob");
                        tracker.remove(&id, &[hash]).await;
                        lifecycle.finish_upload(&base_url, hash, claim, true);
                    }
                    Err(UploadError::Blob(err)) => {
                        tracing::warn!(%hash, ?err, "blob upload failed; relying on announce");
                        lifecycle.finish_upload(&base_url, hash, claim, false);
                    }
                    Err(UploadError::MailboxUnavailable(err)) => {
                        tracing::warn!(%hash, ?err, "mailbox unreachable; aborting remaining uploads, relying on announce/fetch backstop");
                        lifecycle.finish_upload(&base_url, hash, claim, false);
                        claimed.by_ref().for_each(|(hash, claim)| {
                            lifecycle.finish_upload(&base_url, hash, claim, false)
                        });
                        break;
                    }
                }
            }
        });
    }
}

#[async_trait::async_trait]
impl<Item: MailboxItem> MailboxClient<Item> for ToyMailboxClient<Item> {
    fn id(&self) -> MailboxId {
        self.id.clone()
    }

    fn url(&self) -> Option<String> {
        Some(self.base_url.clone())
    }

    async fn publish(&self, ops: Vec<Item>) -> Result<PublishResponse<Item>, anyhow::Error> {
        if ops.is_empty() {
            return Ok(PublishResponse::default());
        }

        // Group operations by topic -> author -> seq_num
        let mut blips: BTreeMap<String, BTreeMap<String, BTreeMap<u64, Blip>>> = BTreeMap::new();

        let blob_hashes: Vec<iroh_blobs::Hash> =
            ops.iter().flat_map(|op| op.blob_hashes()).collect();

        for op in ops {
            let topic_id = op.topic().to_mailbox_key();
            let log_id = op.author().to_mailbox_key();
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
            let response: StoreBlipsResponse = response.json().await?;
            self.store_blobs(blob_hashes).await?;

            let mut result = PublishResponse::default();
            for (topic_str, authors) in response.watermarks {
                let topic = Item::Topic::from_mailbox_key(&topic_str)?;
                for (author_str, watermark) in authors {
                    let author = Item::Author::from_mailbox_key(&author_str)?;
                    result.0.entry(topic).or_default().insert(author, watermark);
                }
            }
            Ok(result)
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

    async fn push_blobs(
        &self,
        hashes: Vec<iroh_blobs::Hash>,
        reader: Arc<dyn crate::BlobReader>,
        tracker: Arc<dyn crate::UnfetchedBlobTracker>,
    ) -> Result<(), anyhow::Error> {
        self.store_blobs_with(hashes, Some(reader), tracker, self.lifecycle.clone())
            .await
    }

    async fn report(&self, request: reporting::ReportRequest) -> Result<(), anyhow::Error> {
        let response = HTTP_CLIENT
            .post(format!("{}/report", self.base_url))
            .json(&request)
            .send()
            .await?;
        if response.status().is_success() {
            Ok(())
        } else {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            Err(anyhow::anyhow!("Failed to report: {} - {}", status, body))
        }
    }

    async fn fetch(
        &self,
        request: FetchRequest<Item>,
    ) -> Result<FetchResponse<Item>, anyhow::Error> {
        // Convert FetchRequest to GetBlipsRequest
        let mut topics: BTreeMap<String, BTreeMap<String, u64>> = BTreeMap::new();

        for (topic, authors) in request.0.iter() {
            let topic_id = topic.to_mailbox_key();
            let mut author_map: BTreeMap<String, u64> = BTreeMap::new();

            for (author, height) in authors.iter() {
                author_map.insert(author.to_mailbox_key(), *height);
            }

            topics.insert(topic_id, author_map);
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
            let topic = Item::Topic::from_mailbox_key(&topic_id_str)?;

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
                let author = Item::Author::from_mailbox_key(&author_str)?;
                missing.insert(author, seq_nums);
            }

            result.insert(topic, FetchTopicResponse { items, missing });
        }

        Ok(FetchResponse(result))
    }
}

impl<Item: MailboxItem> ToyMailboxClient<Item> {
    fn serialize_operation(item: &Item) -> Result<Blip, anyhow::Error> {
        let bytes = p2panda_core::cbor::encode_cbor(item)?;
        Ok(Blip::new(bytes))
    }

    fn deserialize_operation(blip: &Blip) -> Result<Item, anyhow::Error> {
        Ok(p2panda_core::cbor::decode_cbor(blip.as_slice())?)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::{Arc, Mutex};

    use super::*;

    type StoredBlips = BTreeMap<String, BTreeMap<String, BTreeMap<u64, mailbox_server::Blip>>>;

    fn msg(topic: u8, author: char, seq: u64) -> crate::testing::Msg {
        crate::testing::Msg { topic, author, seq }
    }

    /// Spawn a fake mailbox server on a free port. It only stores blips and
    /// echoes them back, plus computes simple contiguous watermarks; the
    /// `missing` vector is always empty because callers only need to verify
    /// that topic/author keys encode and decode correctly through the toy
    /// client. Returns the server's base URL and shared storage.
    async fn spawn_fake_mailbox_server() -> (String, Arc<Mutex<StoredBlips>>) {
        let stored: Arc<Mutex<StoredBlips>> = Arc::new(Mutex::new(BTreeMap::new()));
        let stored_in_store = stored.clone();
        let stored_in_get = stored.clone();

        let app = axum::Router::new()
            .route(
                "/blips/store",
                axum::routing::post(
                    move |axum::Json(req): axum::Json<mailbox_server::StoreBlipsRequest>| async move {
                        let mut stored = stored_in_store.lock().unwrap();
                        let mut watermarks: BTreeMap<String, BTreeMap<String, Option<u64>>> =
                            BTreeMap::new();
                        for (topic, authors) in req.blips {
                            let topic_entry = stored.entry(topic.clone()).or_default();
                            let mut topic_watermarks: BTreeMap<String, Option<u64>> =
                                BTreeMap::new();
                            for (author, seqs) in authors {
                                let author_entry = topic_entry.entry(author.clone()).or_default();
                                for (seq, blip) in &seqs {
                                    author_entry.insert(*seq, blip.clone());
                                }
                                let watermark = seqs
                                    .keys()
                                    .enumerate()
                                    .take_while(|(i, seq)| **seq == *i as u64)
                                    .map(|(_, seq)| seq)
                                    .copied()
                                    .last();
                                topic_watermarks.insert(author, watermark);
                            }
                            watermarks.insert(topic, topic_watermarks);
                        }
                        axum::Json(mailbox_server::StoreBlipsResponse { watermarks })
                    },
                ),
            )
            .route(
                "/blips/get",
                axum::routing::post(
                    move |axum::Json(req): axum::Json<mailbox_server::GetBlipsRequest>| async move {
                        let stored = stored_in_get.lock().unwrap();
                        let mut blips_by_topic: BTreeMap<
                            String,
                            mailbox_server::GetBlipsForTopicResponse,
                        > = BTreeMap::new();
                        for (topic, authors) in req.topics {
                            let mut topic_blips: BTreeMap<String, BTreeMap<u64, mailbox_server::Blip>> =
                                BTreeMap::new();
                            let mut missing: BTreeMap<String, Vec<u64>> = BTreeMap::new();
                            if let Some(topic_entry) = stored.get(&topic) {
                                for (author, min_seq) in authors {
                                    if let Some(author_entry) = topic_entry.get(&author) {
                                        let filtered: BTreeMap<u64, mailbox_server::Blip> =
                                            author_entry
                                                .iter()
                                                .filter(|(seq, _)| **seq > min_seq)
                                                .map(|(seq, blip)| (*seq, blip.clone()))
                                                .collect();
                                        topic_blips.insert(author, filtered);
                                    } else {
                                        missing.insert(author, Vec::new());
                                    }
                                }
                            } else {
                                for (author, _) in authors {
                                    missing.insert(author, Vec::new());
                                }
                            }
                            blips_by_topic.insert(
                                topic,
                                mailbox_server::GetBlipsForTopicResponse {
                                    blips: topic_blips,
                                    missing,
                                },
                            );
                        }
                        axum::Json(mailbox_server::GetBlipsResponse { blips_by_topic })
                    },
                ),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let base_url = format!("http://{addr}");
        (base_url, stored)
    }

    #[tokio::test]
    async fn register_hashes_records_not_stored_and_removes_already_stored() {
        // Server that reports h_stored as already stored, h_new as not.
        let h_stored = iroh_blobs::Hash::new([1; 32]);
        let h_new = iroh_blobs::Hash::new([2; 32]);
        let app = axum::Router::new().route(
            "/blobs/register-hashes",
            axum::routing::post(
                move |axum::Json(req): axum::Json<mailbox_server::RegisterHashesRequest>| async move {
                    let already_stored: Vec<_> = req
                        .blob_hashes
                        .into_iter()
                        .filter(|h| *h == h_stored)
                        .collect();
                    axum::Json(mailbox_server::RegisterHashesResponse { already_stored })
                },
            ),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let base_url = format!("http://{addr}");

        let already = send_register_hashes(
            &base_url,
            vec![h_stored, h_new],
            iroh::SecretKey::from_bytes(&[3; 32]).public(),
            false,
        )
        .await
        .unwrap();
        assert_eq!(already, vec![h_stored]);
    }

    #[tokio::test]
    async fn publish_and_fetch_round_trip_msg_keys() {
        let (base_url, stored) = spawn_fake_mailbox_server().await;

        let client = ToyMailboxClient::<crate::testing::Msg>::new(
            "mbx".to_string(),
            base_url,
            iroh::SecretKey::from_bytes(&[3; 32]).public(),
            std::sync::Arc::new(crate::NoopUnfetchedBlobTracker),
        );

        let response = client
            .publish(vec![msg(42, 'a', 0), msg(42, 'a', 1), msg(42, 'b', 0)])
            .await
            .unwrap();

        // The watermark response decoded the string keys back to Msg's Topic
        // and Author types.
        assert_eq!(response.watermark(&42, &'a'), Some(1));
        assert_eq!(response.watermark(&42, &'b'), Some(0));

        // The keys were encoded as bare strings, not JSON arrays/objects.
        let stored = stored.lock().unwrap();
        assert_eq!(stored.keys().collect::<Vec<_>>(), vec!["42"]);
        let author_keys: std::collections::BTreeSet<&str> =
            stored["42"].keys().map(|s| s.as_str()).collect();
        assert_eq!(author_keys, ["a", "b"].iter().copied().collect());
        drop(stored);

        // Fetch back the items above the reported local height.
        let request = FetchRequest(BTreeMap::from([(
            42u8,
            BTreeMap::from([('a', 0u64), ('b', 0u64)]),
        )]));
        let response = client.fetch(request).await.unwrap();

        let topic_response = response.0.get(&42).expect("topic 42");
        let author_a_seqs: Vec<u64> = topic_response
            .items
            .iter()
            .filter(|m| m.author() == 'a')
            .map(|m| m.seq_num())
            .collect();
        let author_b_seqs: Vec<u64> = topic_response
            .items
            .iter()
            .filter(|m| m.author() == 'b')
            .map(|m| m.seq_num())
            .collect();
        assert_eq!(author_a_seqs, vec![1]);
        assert!(author_b_seqs.is_empty());
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
        upload_blob(&base_url, data.clone()).await.unwrap();
        assert_eq!(*received.lock().unwrap(), data.to_vec());
    }

    #[tokio::test]
    async fn upload_blob_reports_mailbox_unavailable_when_unreachable() {
        // Bind then drop the listener so the port is closed and the connect fails.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);
        let base_url = format!("http://{addr}");

        let err = upload_blob(&base_url, bytes::Bytes::from_static(b"x"))
            .await
            .unwrap_err();
        assert!(matches!(err, UploadError::MailboxUnavailable(_)));
    }

    #[tokio::test]
    async fn upload_blob_reports_blob_failure_on_error_status() {
        let app = axum::Router::new().route(
            "/blobs/upload",
            axum::routing::post(|| async {
                (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "boom")
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let base_url = format!("http://{addr}");

        let err = upload_blob(&base_url, bytes::Bytes::from_static(b"x"))
            .await
            .unwrap_err();
        assert!(matches!(err, UploadError::Blob(_)));
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
            async fn has_blob(&self, _hash: iroh_blobs::Hash) -> bool {
                true
            }
        }

        let data = bytes::Bytes::from_static(b"a blob");
        let hash = iroh_blobs::Hash::new(&data);

        // The mailbox has nothing yet, so the announce reports no `already_stored`
        // and the client streams the bytes to `/blobs/upload` afterward.
        let uploaded: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
        let uploaded_in_handler = uploaded.clone();
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
                "/blobs/register-hashes",
                axum::routing::post(
                    |axum::Json(_req): axum::Json<mailbox_server::RegisterHashesRequest>| async move {
                        axum::Json(mailbox_server::RegisterHashesResponse {
                            already_stored: Vec::new(),
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

        let client = ToyMailboxClient::<crate::testing::Msg>::new(
            "mbx".to_string(),
            base_url,
            iroh::SecretKey::from_bytes(&[3; 32]).public(),
            std::sync::Arc::new(crate::NoopUnfetchedBlobTracker),
        )
        .with_blob_reader(std::sync::Arc::new(StubReader(data.clone())));

        client.store_blobs(vec![hash]).await.unwrap();

        // The upload is spawned, so wait for the detached task to deliver it.
        for _ in 0..100 {
            if *uploaded.lock().unwrap() == data.to_vec() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        assert_eq!(*uploaded.lock().unwrap(), data.to_vec());
    }
}
