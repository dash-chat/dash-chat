//! Manages syncing blobs referenced in logs over iroh-blobs

use futures::FutureExt;
use iroh_blobs::api::downloader::{
    DownloadProgressItem, DownloadRequest, Downloader, FiniteRequest, SplitStrategy,
};
use iroh_blobs::protocol::GetRequest;
use mailbox_client::manager::Mailboxes;
use p2panda::operation::LogId;
use std::panic::AssertUnwindSafe;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::Duration,
};
use tokio::sync::Notify;
use tokio::task::{JoinHandle, JoinSet};
use tokio_stream::StreamExt;

pub use crate::blob_fetch::BlobFetchConfig;

use crate::blob_fetch::MissingBlobs;
use crate::{DeviceId, TopicId, mailbox::MailboxOperation, stores::OpStore};
use dashchat_utils::blob_sync::MAX_BLOB_BYTES;

const RESCAN_DEBOUNCE: Duration = Duration::from_millis(100);

/// Manages syncing blobs referenced in logs over iroh-blobs
#[derive(Clone)]
pub struct BlobSync {
    pub blobs: iroh_blobs::BlobsProtocol,
    downloader: Downloader,
    op_store: OpStore,
    mailboxes: Mailboxes<MailboxOperation, OpStore>,
    self_endpoint: iroh::EndpointId,
    tags_changed: Arc<Notify>,
    fetch_loop: Arc<std::sync::Mutex<Option<JoinHandle<()>>>>,
}

impl BlobSync {
    /// Also starts the background loop that fetches missing blobs, which runs
    /// until [`Self::shutdown`].
    pub async fn new(
        endpoint: p2panda::Endpoint,
        store: iroh_blobs::store::fs::FsStore,
        op_store: OpStore,
        mailboxes: Mailboxes<MailboxOperation, OpStore>,
        fetch_config: BlobFetchConfig,
    ) -> anyhow::Result<Self> {
        // Accepts pushes too, which a node hosting a local hub relies on:
        // iroh-blobs gates every request kind on `mask.get` (n0-computer/iroh-blobs#250).
        let blobs = iroh_blobs::BlobsProtocol::new(&store, None);
        let mixed_alpn =
            p2panda_net::hash_protocol_id_with_network_id(iroh_blobs::ALPN, endpoint.network_id());
        endpoint.accept(iroh_blobs::ALPN, blobs.clone()).await?;
        let iroh_endpoint = endpoint.endpoint().await?;
        let downloader =
            Downloader::new_with_opts(&store, &iroh_endpoint, &mixed_alpn, Default::default());

        let blob_sync = Self {
            blobs,
            downloader,
            op_store,
            mailboxes,
            self_endpoint: iroh_endpoint.id(),
            tags_changed: Default::default(),
            fetch_loop: Default::default(),
        };
        let this = blob_sync.clone();
        let fetch_loop = tokio::spawn(async move { this.fetch_loop(fetch_config).await });
        *blob_sync.fetch_loop.lock().unwrap() = Some(fetch_loop);
        Ok(blob_sync)
    }

    /// Stop the fetch loop and close the blob store, releasing its file lock.
    pub async fn shutdown(&self) {
        let fetch_loop = self.fetch_loop.lock().unwrap().take();
        if let Some(fetch_loop) = fetch_loop {
            fetch_loop.abort();
            let _ = fetch_loop.await;
        }
        match tokio::time::timeout(Duration::from_secs(5), self.blobs.store().shutdown()).await {
            Ok(Ok(())) => {}
            Ok(Err(err)) => tracing::warn!("failed to shut down blob store: {err:?}"),
            Err(_) => tracing::warn!("timed out shutting down blob store"),
        }
    }

    async fn fetch_loop(&self, config: BlobFetchConfig) {
        let concurrency = config.concurrency.max(1);
        let mut missing = MissingBlobs::default();
        let mut in_flight = JoinSet::new();
        let mut fetching = HashSet::new();
        let mut rescan = true;
        loop {
            if rescan {
                match self.missing_blobs().await {
                    Ok(topics) => {
                        missing.replace(topics);
                        rescan = false;
                    }
                    Err(err) => {
                        tracing::warn!(?err, "failed to list missing blobs");
                        tokio::time::sleep(config.min_retry_interval).await;
                        continue;
                    }
                }
            }
            for (hash, topics) in missing.due(&fetching, concurrency - in_flight.len()) {
                fetching.insert(hash);
                let this = self.clone();
                let attempt_timeout = config.attempt_timeout;
                // Caught, so a panicking fetch still reports its hash back and
                // is retried rather than counted as fetching forever.
                in_flight.spawn(async move {
                    let fetch = this.try_fetch(&topics, hash, attempt_timeout);
                    let fetched = AssertUnwindSafe(fetch).catch_unwind().await;
                    (hash, fetched.unwrap_or(false))
                });
            }
            // With every slot taken, nothing can start before a fetch finishes.
            let next_attempt = match in_flight.len() < concurrency {
                true => missing.next_attempt(&fetching),
                false => None,
            };
            tokio::select! {
                Some(Ok((hash, fetched))) = in_flight.join_next() => {
                    fetching.remove(&hash);
                    if fetched {
                        missing.fetched(hash);
                    } else {
                        missing.failed(hash, &config);
                    }
                }
                _ = sleep_until(next_attempt) => {}
                _ = self.tags_changed.notified() => {
                    // Lets a burst of new tags, as when history syncs in, share
                    // one rescan.
                    tokio::time::sleep(RESCAN_DEBOUNCE).await;
                    rescan = true;
                }
            }
        }
    }

    /// Every blob a stored message references is tagged before it is fetched,
    /// so the tagged blobs this device lacks are the ones left to fetch.
    async fn missing_blobs(&self) -> anyhow::Result<HashMap<iroh_blobs::Hash, Vec<TopicId>>> {
        let mut missing = HashMap::new();
        for (hash, topics) in self.referencing_topics().await? {
            if !self.blobs.has(hash).await? {
                missing.insert(hash, topics);
            }
        }
        Ok(missing)
    }

    /// The topics whose messages reference each tagged blob.
    async fn referencing_topics(&self) -> anyhow::Result<HashMap<iroh_blobs::Hash, Vec<TopicId>>> {
        let mut tags = self.blobs.store().tags().list().await?;
        let mut topics: HashMap<iroh_blobs::Hash, Vec<TopicId>> = HashMap::new();
        while let Some(tag) = tags.next().await {
            let tag = tag?;
            let Some(topic) = blob_tag_topic(tag.name.as_ref()) else {
                continue;
            };
            let referencing = topics.entry(tag.hash).or_default();
            if !referencing.contains(&topic) {
                referencing.push(topic);
            }
        }
        Ok(topics)
    }

    /// The topics whose messages reference `hash`, which its sources are
    /// looked up from.
    pub async fn topics_for(&self, hash: iroh_blobs::Hash) -> anyhow::Result<Vec<TopicId>> {
        Ok(self
            .referencing_topics()
            .await?
            .remove(&hash)
            .unwrap_or_default())
    }

    /// Attempt to fetch a single blob from the sources of the `topics` it was
    /// referenced in, returning `true` when it is present in the local store
    /// afterwards (already cached or newly downloaded).
    async fn try_fetch(
        &self,
        topics: &[TopicId],
        hash: iroh_blobs::Hash,
        attempt_timeout: Duration,
    ) -> bool {
        if self.blobs.has(hash).await.unwrap_or(false) {
            return true;
        }

        let sources = match self.sources(topics).await {
            Ok(sources) => sources,
            Err(err) => {
                tracing::warn!(%hash, ?err, "blob source lookup failed");
                return false;
            }
        };

        if sources.is_empty() {
            tracing::debug!(%hash, "no blob sources");
            return false;
        }

        let source_count = sources.len();
        let fetched = self.download_from(hash, sources, attempt_timeout).await;
        tracing::debug!(%hash, source_count, fetched, "blob fetch attempt");
        fetched
    }

    /// Download `hash` from `providers`, aborting if the transfer exceeds
    /// [`MAX_BLOB_BYTES`]. Returns whether the blob is present locally afterwards.
    pub async fn download_from(
        &self,
        hash: iroh_blobs::Hash,
        providers: Vec<iroh::EndpointId>,
        timeout: Duration,
    ) -> bool {
        let result = tokio::time::timeout(timeout, async {
            let options = DownloadRequest {
                // Media are single blobs, not hash-sequences. `GetRequest::blob`
                // requests only the blob itself; `GetRequest::all` would additionally
                // request the blob's hash-sequence children, which makes the provider
                // parse the raw media bytes as a hash-seq and reset the stream with
                // `ERR_INTERNAL` (`InvalidHashSeq`) — so the blob never transfers.
                request: FiniteRequest::Get(GetRequest::blob(hash)),
                providers: Arc::new(providers),
                // `SplitStrategy::Split` routes the download through iroh-blobs'
                // hash-seq path, which asserts the root size is a multiple of 32 and
                // so always fails for a raw blob ("Size is not a multiple of 32").
                strategy: SplitStrategy::None,
            };
            let mut stream = self
                .downloader
                .download_with_opts(options)
                .stream()
                .await
                .map_err(|e| anyhow::anyhow!("download stream: {e}"))?;
            while let Some(item) = stream.next().await {
                match item {
                    // Dropping the stream on return cancels the in-flight download.
                    DownloadProgressItem::Progress(total) if total > MAX_BLOB_BYTES => {
                        anyhow::bail!("blob exceeds {MAX_BLOB_BYTES} byte cap ({total} bytes)")
                    }
                    DownloadProgressItem::Error(err) => anyhow::bail!("download failed: {err}"),
                    DownloadProgressItem::DownloadError => anyhow::bail!("download error"),
                    _ => {}
                }
            }
            anyhow::Ok(())
        })
        .await;

        match result {
            Ok(Ok(())) => self.blobs.has(hash).await.unwrap_or(false),
            Ok(Err(err)) => {
                tracing::debug!(%hash, ?err, "blob download failed");
                false
            }
            Err(_) => {
                tracing::warn!(%hash, "blob download timed out");
                false
            }
        }
    }

    /// Tag a blob a processed message references, keeping it and, until this
    /// device holds it, queueing it to be fetched.
    pub async fn queue_blob_fetch(
        &self,
        topic: TopicId,
        author: DeviceId,
        operation_hash: p2panda::Hash,
        blob_hash: iroh_blobs::Hash,
    ) -> anyhow::Result<()> {
        // Protect the blob with the tag before fetching.
        // This is the right moment to do it, because if multiple authors
        // publish the same blob, we want tags from each of them
        let tag_name = blob_tag_name(topic, author, operation_hash, blob_hash);
        self.blobs.store().tags().set(tag_name, blob_hash).await?;
        self.tags_changed.notify_one();
        tracing::debug!(hash = %blob_hash, "queued blob for fetch");
        Ok(())
    }

    /// Store the bytes of media this device is sending. The returned temp tag
    /// keeps them until the message referencing them is processed, which tags
    /// them like any received media.
    ///
    /// Refuses media larger than [`MAX_BLOB_BYTES`], so an honest node never
    /// references a blob that the fetcher's own cap would reject.
    pub async fn store_blob(
        &self,
        data: impl Into<bytes::Bytes>,
    ) -> anyhow::Result<iroh_blobs::api::TempTag> {
        let data = data.into();
        if data.len() as u64 > MAX_BLOB_BYTES {
            anyhow::bail!(
                "a media item is {} bytes, exceeds {MAX_BLOB_BYTES} byte limit",
                data.len()
            );
        }
        Ok(self.blobs.blobs().add_bytes(data).temp_tag().await?)
    }

    /// Delete all tags for the given `(topic, author, operation_hash, blob_hash)` tuples,
    /// allowing iroh's GC to reclaim data no longer referenced.
    pub async fn delete_blobs(
        &self,
        topic: TopicId,
        author: DeviceId,
        operation_hash: p2panda::Hash,
        blob_hashes: impl IntoIterator<Item = iroh_blobs::Hash>,
    ) {
        let tags = self.blobs.store().tags();
        for hash in blob_hashes {
            let tag_name = blob_tag_name(topic, author, operation_hash, hash);
            if let Err(err) = tags.delete(tag_name).await {
                tracing::warn!(?err, "failed to delete blob tag");
            }
        }
        self.tags_changed.notify_one();
    }

    /// Keep attempting an on-demand download of `hash` until it is present
    /// locally or `timeout` elapses, bypassing the background loop's backoff.
    /// Retries within the window so a fast-failing attempt — e.g. a momentarily
    /// unreachable provider — gets another chance, and keeps watching the store
    /// so a blob arriving by another path counts too. Concurrent downloads of
    /// the same hash are coalesced by the iroh-blobs downloader, so racing the
    /// background loop is safe.
    pub async fn fetch_now(&self, hash: iroh_blobs::Hash, timeout: Duration) -> bool {
        if self.blobs.has(hash).await.unwrap_or(false) {
            return true;
        }
        let deadline = std::time::Instant::now() + timeout;
        let topics = self.topics_for(hash).await.unwrap_or_default();
        loop {
            if self.blobs.has(hash).await.unwrap_or(false) {
                return true;
            }
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                return false;
            }
            if !topics.is_empty() && self.try_fetch(&topics, hash, remaining).await {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    }

    /// Providers for blobs referenced in `topics`: mailboxes first, then the
    /// topics' authors.
    async fn sources(&self, topics: &[TopicId]) -> anyhow::Result<Vec<iroh::EndpointId>> {
        let mut sources = vec![];
        for topic in topics {
            sources.extend(self.mailboxes.get_sources(topic).await?);
        }
        for topic in topics {
            for author in self.op_store.get_authors(LogId::from_topic(*topic)).await? {
                sources.push(iroh::EndpointId::from_bytes(author.as_bytes())?);
            }
        }
        // Never dial ourselves (we already early-return when the blob is local),
        // and dedupe so a provider isn't dialed twice — redundant dials churn
        // iroh connection paths.
        let mut seen = HashSet::new();
        sources.retain(|id| *id != self.self_endpoint && seen.insert(*id));
        Ok(sources)
    }
}

async fn sleep_until(at: Option<tokio::time::Instant>) {
    match at {
        Some(at) => tokio::time::sleep_until(at).await,
        None => std::future::pending().await,
    }
}

fn blob_tag_name(
    topic: TopicId,
    author: DeviceId,
    operation_hash: p2panda::Hash,
    blob_hash: iroh_blobs::Hash,
) -> Vec<u8> {
    let mut name = Vec::with_capacity(128);
    name.extend_from_slice(topic.as_bytes());
    name.extend_from_slice(author.as_bytes());
    name.extend_from_slice(operation_hash.as_bytes());
    name.extend_from_slice(blob_hash.as_bytes());
    name
}

fn blob_tag_topic(name: &[u8]) -> Option<TopicId> {
    let topic: [u8; 32] = name.get(..32)?.try_into().ok()?;
    Some(TopicId::from(topic))
}

#[cfg(test)]
mod tests {
    /// Collect all persistent tag names from a blob store.
    async fn list_tag_names(blobs: &iroh_blobs::BlobsProtocol) -> Vec<Vec<u8>> {
        use tokio_stream::StreamExt;
        let stream = blobs.store().tags().list().await.unwrap();
        tokio::pin!(stream);
        let mut names = vec![];
        while let Some(Ok(info)) = stream.next().await {
            names.push(info.name.0.to_vec());
        }
        names
    }

    async fn tag_count_for_hash(
        blobs: &iroh_blobs::BlobsProtocol,
        hash: iroh_blobs::Hash,
    ) -> usize {
        list_tag_names(blobs)
            .await
            .into_iter()
            .filter(|name| name.ends_with(hash.as_bytes()))
            .count()
    }

    #[cfg(feature = "testing")]
    mod integration {
        use super::*;
        use crate::{DeviceId, NodeConfig, testing::TestNode, topic::TopicId};

        /// Store `data` on `node` tagged as `author`'s, as processing a message
        /// of theirs referencing it does.
        async fn store_tagged(
            node: &TestNode,
            topic: TopicId,
            author: DeviceId,
            op_hash: p2panda::Hash,
            data: &[u8],
        ) -> iroh_blobs::Hash {
            let hash = node.blobs().add_bytes(data.to_vec()).await.unwrap().hash;
            node.blob_sync()
                .queue_blob_fetch(topic, author, op_hash, hash)
                .await
                .unwrap();
            hash
        }

        #[tokio::test(flavor = "multi_thread")]
        async fn two_authors_same_blob_get_distinct_tags() {
            let alice = TestNode::new(NodeConfig::testing(), "alice").await;
            let bobbi = TestNode::new(NodeConfig::testing(), "bobbi").await;
            let topic = TopicId::random();
            let operation_hash = p2panda::Hash::digest(b"test-op");
            let data = b"shared-media-blob";

            let hash_alice =
                store_tagged(&alice, topic, alice.device_id(), operation_hash, data).await;
            let hash_bobbi =
                store_tagged(&bobbi, topic, bobbi.device_id(), operation_hash, data).await;

            // Same content → same hash.
            assert_eq!(hash_alice, hash_bobbi);

            // Each node has exactly one tag for that hash (their own authorship tag).
            assert_eq!(tag_count_for_hash(&alice.blobs(), hash_alice).await, 1);
            assert_eq!(tag_count_for_hash(&bobbi.blobs(), hash_bobbi).await, 1);
        }

        #[tokio::test(flavor = "multi_thread")]
        async fn deleting_alice_author_tag_leaves_one_tag_each() {
            let alice = TestNode::new(NodeConfig::testing(), "alice").await;
            let bobbi = TestNode::new(NodeConfig::testing(), "bobbi").await;
            let topic = TopicId::random();
            let op_hash = p2panda::Hash::digest(b"test-op");
            let data = b"shared-media-blob";

            // Both nodes store the same blob under their own authorship.
            let hash = store_tagged(&alice, topic, alice.device_id(), op_hash, data).await;
            store_tagged(&alice, topic, bobbi.device_id(), op_hash, data).await;
            store_tagged(&bobbi, topic, alice.device_id(), op_hash, data).await;
            store_tagged(&bobbi, topic, bobbi.device_id(), op_hash, data).await;

            // Before deletion: two tags each.
            assert_eq!(tag_count_for_hash(&alice.blobs(), hash).await, 2);
            assert_eq!(tag_count_for_hash(&bobbi.blobs(), hash).await, 2);

            // Delete alice's authorship tag on both nodes.
            alice
                .blob_sync()
                .delete_blobs(topic, alice.device_id(), op_hash, [hash])
                .await;
            bobbi
                .blob_sync()
                .delete_blobs(topic, alice.device_id(), op_hash, [hash])
                .await;

            // One tag remains on each node (bobbi's).
            assert_eq!(tag_count_for_hash(&alice.blobs(), hash).await, 1);
            assert_eq!(tag_count_for_hash(&bobbi.blobs(), hash).await, 1);
        }

        #[tokio::test(flavor = "multi_thread")]
        async fn deleting_both_author_tags_leaves_no_tags() {
            let alice = TestNode::new(NodeConfig::testing(), "alice").await;
            let bobbi = TestNode::new(NodeConfig::testing(), "bobbi").await;
            let topic = TopicId::random();
            let op_hash = p2panda::Hash::digest(b"test-op");
            let data = b"gc-target-blob";

            let hash = store_tagged(&alice, topic, alice.device_id(), op_hash, data).await;
            store_tagged(&alice, topic, bobbi.device_id(), op_hash, data).await;
            store_tagged(&bobbi, topic, alice.device_id(), op_hash, data).await;
            store_tagged(&bobbi, topic, bobbi.device_id(), op_hash, data).await;

            // Delete all tags on both nodes.
            for node in [&alice, &bobbi] {
                node.blob_sync()
                    .delete_blobs(topic, alice.device_id(), op_hash, [hash])
                    .await;
                node.blob_sync()
                    .delete_blobs(topic, bobbi.device_id(), op_hash, [hash])
                    .await;
            }

            // No pinning tags remain — blob is eligible for GC on the next cycle.
            assert_eq!(tag_count_for_hash(&alice.blobs(), hash).await, 0);
            assert_eq!(tag_count_for_hash(&bobbi.blobs(), hash).await, 0);
        }
    }
}
