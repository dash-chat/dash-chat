//! Manages syncing blobs referenced in logs over iroh-blobs

use aliased::Aliasing;
use derive_more::derive::Constructor;
use futures::Stream;
use iroh_blobs::api::downloader::{Downloader, Shuffled};
use mailbox_client::manager::Mailboxes;
use p2panda::operation::{LogId, Operation};
use p2panda_store::{SqliteStore, topics::TopicStore};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::Arc,
    time::Duration,
};
use tokio::{
    sync::{Mutex, Notify},
    task::JoinHandle,
};
use tokio_stream::StreamExt;

use dashchat_utils::FetchPool;

pub use dashchat_utils::FetchConfig as BlobFetchConfig;

use crate::{
    AsBody, ChatPayload, DeviceId, Payload, TopicId, mailbox::MailboxOperation, stores::OpStore,
};

/// Drop a pending fetch entry after this many consecutive failed passes, so a
/// permanently-unfetchable blob doesn't accumulate steady-state background work.
const MAX_FETCH_FAILURES: u32 = 10;

/// Manages syncing blobs referenced in logs over iroh-blobs
#[derive(Clone)]
pub struct BlobSync {
    pub blobs: iroh_blobs::BlobsProtocol,
    pub fetch_pool: BlobFetchPool,
    pub sources: MixedSourceLookup,
    downloader: Downloader,
}

impl BlobSync {
    pub async fn new(
        endpoint: p2panda::Endpoint,
        root: PathBuf,
        blob_fetch: BlobFetchPool,
        sources: MixedSourceLookup,
    ) -> anyhow::Result<Self> {
        let store = iroh_blobs::store::fs::FsStore::load(root).await?;
        let blobs = iroh_blobs::BlobsProtocol::new(&store, None);
        let mixed_alpn =
            p2panda_net::hash_protocol_id_with_network_id(iroh_blobs::ALPN, endpoint.network_id());
        endpoint.accept(iroh_blobs::ALPN, blobs.clone()).await?;
        let downloader = Downloader::new_with_opts(
            &store,
            &endpoint.endpoint().await?,
            &mixed_alpn,
            Default::default(),
        );

        Ok(Self {
            blobs,
            fetch_pool: blob_fetch,
            sources,
            downloader,
        })
    }

    /// Clone the downloader so an in-process mailbox can fetch blobs into this
    /// node's shared blob store.
    pub fn downloader(&self) -> Downloader {
        self.downloader.clone()
    }

    /// Spawn the background loop that drains the fetch pool, returning a handle
    /// that cancels the loop when aborted.
    pub fn spawn_fetch_loop(&self, config: BlobFetchConfig) -> JoinHandle<()> {
        let this = self.clone();
        let pool = self.fetch_pool.clone();
        tokio::spawn(dashchat_utils::fetch_loop(
            pool,
            config,
            move |(topic, hash), attempt_timeout| {
                let this = this.clone();
                async move { this.try_fetch(topic, hash, attempt_timeout).await }
            },
        ))
    }

    /// Attempt to fetch a single blob, returning `true` when it is present in
    /// the local store afterwards (already cached or newly downloaded).
    async fn try_fetch(
        &self,
        topic: TopicId,
        hash: iroh_blobs::Hash,
        attempt_timeout: Duration,
    ) -> bool {
        if self.blobs.has(hash).await.unwrap_or(false) {
            return true;
        }

        let sources = match self.sources.sources(topic).await {
            Ok(sources) => sources,
            Err(err) => {
                tracing::warn!(%hash, ?err, "blob source lookup failed");
                return false;
            }
        };

        if sources.is_empty() {
            return false;
        }

        let providers = Shuffled::new(sources.into_iter().map(Into::into).collect());
        dashchat_utils::blob_sync::download_capped(
            &self.downloader,
            hash,
            providers,
            attempt_timeout,
            &self.blobs,
        )
        .await
    }

    pub async fn add_to_fetch_pool(
        &self,
        topic: TopicId,
        author: DeviceId,
        hash: iroh_blobs::Hash,
    ) -> anyhow::Result<()> {
        // Protect the blob with the tag before fetching.
        // This is the right moment to do it, because if multiple authors
        // publish the same blob, we want tags from each of them
        let tag_name = blob_tag_name(topic, author, hash);
        self.blobs.store().tags().set(tag_name, hash).await?;
        self.fetch_pool.add(topic, hash).await;
        Ok(())
    }

    /// Store blob bytes and tag them with a name that encodes `(topic, author, hash)`
    /// so deletion can be scoped to a specific topic.
    pub async fn store_blob(
        &self,
        topic: TopicId,
        author: DeviceId,
        data: impl Into<bytes::Bytes>,
    ) -> anyhow::Result<iroh_blobs::Hash> {
        let tt = self.blobs.blobs().add_bytes(data).temp_tag().await?;
        let hash = tt.hash();
        let tag_name = blob_tag_name(topic, author, hash);
        self.blobs
            .store()
            .tags()
            .set(tag_name, tt.hash_and_format())
            .await?;
        Ok(hash)
    }

    /// Delete all tags for the given `(topic, author, hash)` pairs, allowing iroh's GC
    /// to reclaim blob data that is no longer referenced by any topic + author.
    pub async fn delete_blobs(
        &self,
        topic: TopicId,
        author: DeviceId,
        hashes: impl IntoIterator<Item = iroh_blobs::Hash>,
    ) {
        let tags = self.blobs.store().tags();
        for hash in hashes {
            let tag_name = blob_tag_name(topic, author, hash);
            if let Err(err) = tags.delete(tag_name).await {
                tracing::warn!(?err, "failed to delete blob tag");
            }
            self.fetch_pool.remove(topic, hash).await;
        }
    }

    /// Keep attempting an on-demand download of `hash` until it is present
    /// locally or `timeout` elapses, bypassing the background loop's long pass
    /// interval (up to a minute away). Retries within the window so a
    /// fast-failing attempt — e.g. a momentarily unreachable provider — gets
    /// another chance instead of leaving the caller to wait out the window.
    /// Tries every topic the pool associates with the hash; concurrent
    /// downloads of the same hash are coalesced by the iroh-blobs downloader,
    /// so racing the background loop is safe.
    pub async fn fetch_now(&self, hash: iroh_blobs::Hash, timeout: Duration) -> bool {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            if self.blobs.has(hash).await.unwrap_or(false) {
                return true;
            }
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                return false;
            }
            let topics = self.fetch_pool.topics_for(hash).await;
            // No known source yet; the caller's poll still waits out the window
            // in case the blob arrives via the background loop or a mailbox.
            if topics.is_empty() {
                return false;
            }
            for topic in topics {
                if self.try_fetch(topic, hash, remaining).await {
                    return true;
                }
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }
}

fn blob_tag_name(topic: TopicId, author: DeviceId, hash: iroh_blobs::Hash) -> Vec<u8> {
    let mut name = Vec::with_capacity(96);
    name.extend_from_slice(topic.as_bytes());
    name.extend_from_slice(author.as_bytes());
    name.extend_from_slice(hash.as_bytes());
    name
}

#[derive(Clone, Default)]
pub struct BlobFetchPool {
    stack: Arc<Mutex<Vec<(TopicId, iroh_blobs::Hash)>>>,
    /// Consecutive failed-pass count per hash. A hash that can never be fetched
    /// (sender deleted it, provider permanently gone, garbage hash) is evicted
    /// after [`MAX_FETCH_FAILURES`] so it isn't re-attempted every pass forever.
    failures: Arc<Mutex<HashMap<iroh_blobs::Hash, u32>>>,
    added: Arc<Notify>,
}

#[async_trait::async_trait]
impl FetchPool for BlobFetchPool {
    type Item = (TopicId, iroh_blobs::Hash);
    type Key = iroh_blobs::Hash;

    fn key(item: &Self::Item) -> Self::Key {
        item.1
    }
    async fn is_empty(&self) -> bool {
        self.stack.lock().await.is_empty()
    }
    async fn next_untried(&self, tried: &HashSet<iroh_blobs::Hash>) -> Option<Self::Item> {
        let stack = self.stack.lock().await;
        stack
            .iter()
            .rev()
            .find(|(_, hash)| !tried.contains(hash))
            .copied()
    }
    async fn remove(&self, item: &Self::Item) {
        self.stack.lock().await.retain(|entry| entry != item);
        self.failures.lock().await.remove(&item.1);
    }
    async fn on_failure(&self, item: &Self::Item) {
        let hash = item.1;
        let mut failures = self.failures.lock().await;
        let count = failures.entry(hash).or_insert(0);
        *count += 1;
        if *count >= MAX_FETCH_FAILURES {
            failures.remove(&hash);
            drop(failures);
            self.stack.lock().await.retain(|(_, h)| *h != hash);
            tracing::debug!(%hash, "evicting blob from fetch pool after too many failures");
        }
    }
    async fn wait_for_add(&self) {
        self.added.notified().await;
    }
}

impl BlobFetchPool {
    // Not pub so that we call it from BlobSync and add a tag at the same time.
    async fn add(&self, topic: TopicId, hash: iroh_blobs::Hash) {
        // A fresh reference resets the failure count, giving an on-demand
        // `load_blob` (or a new message) another full round of attempts.
        self.failures.lock().await.remove(&hash);
        self.stack.lock().await.push((topic, hash));
        self.added.notify_one();
    }

    // Not pub so that we call it from BlobSync and remove a tag at the same time.
    async fn remove(&self, topic: TopicId, hash: iroh_blobs::Hash) {
        self.stack
            .lock()
            .await
            .retain(|(t, h)| *t != topic || *h != hash);
    }

    /// Topics the pool currently associates with `hash`, used to resolve blob
    /// sources for an on-demand fetch.
    pub async fn topics_for(&self, hash: iroh_blobs::Hash) -> Vec<TopicId> {
        self.stack
            .lock()
            .await
            .iter()
            .filter(|(_, h)| *h == hash)
            .map(|(topic, _)| *topic)
            .collect()
    }

    /// Build a fetch pool from a stream of stored operations.
    ///
    /// The `topic_for_log_id` closure maps each operation's log_id back to a
    /// `TopicId`. `LogId = blake3(topic.as_bytes())` is one-way, so callers
    /// must supply this mapping from their own store. Returns `None` to skip
    /// an operation whose topic cannot be recovered.
    pub async fn from_ops(
        ops: impl Stream<Item = Result<Operation, anyhow::Error>> + '_,
        topic_store: SqliteStore,
    ) -> anyhow::Result<Self> {
        let store = Self::default();
        let mut s = store.stack.lock().await;
        tokio::pin!(ops);
        while let Some(op) = ops.try_next().await? {
            let Some(body) = op.body else {
                continue;
            };
            let Ok(payload) = Payload::try_from_body(&body) else {
                continue;
            };
            match payload {
                Payload::Chat(ChatPayload::Message(m)) => {
                    if let Some(media) = m.media() {
                        let Some(topic) = topic_store
                            .resolve_topic(&op.header.verifying_key, &op.header.extensions.log_id)
                            .await?
                        else {
                            tracing::error!(
                                author = ?op.header.verifying_key.aliased(),
                                log_id = ?op.header.extensions.log_id.aliased(),
                                "failed to resolve topic for operation",
                            );
                            continue;
                        };
                        for item in media {
                            s.push((topic, item.hash));
                        }
                    }
                }
                _ => continue,
            }
        }
        drop(s);
        Ok(store)
    }
}

#[derive(Clone, Constructor)]
pub struct MixedSourceLookup {
    op_store: OpStore,
    mailboxes: Mailboxes<MailboxOperation, OpStore>,
    self_endpoint: iroh::EndpointId,
}

impl MixedSourceLookup {
    pub async fn sources(&self, topic: TopicId) -> anyhow::Result<Vec<iroh::EndpointId>> {
        let log_id = LogId::from_topic(topic);
        let mut sources = self
            .op_store
            .get_authors(log_id)
            .await?
            .into_iter()
            .map(|author| iroh::EndpointId::from_bytes(author.as_bytes()))
            .collect::<Result<Vec<iroh::EndpointId>, _>>()?;
        sources.extend(self.mailboxes.get_sources(&topic).await?);
        // Never dial ourselves (we already early-return when the blob is local),
        // and dedupe so a provider isn't dialed twice — redundant dials churn
        // iroh connection paths.
        let mut seen = HashSet::new();
        sources.retain(|id| *id != self.self_endpoint && seen.insert(*id));
        Ok(sources)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(n: u8) -> iroh_blobs::Hash {
        iroh_blobs::Hash::new([n; 32])
    }

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
        use crate::{NodeConfig, testing::TestNode, topic::TopicId};

        #[tokio::test(flavor = "multi_thread")]
        async fn two_authors_same_blob_get_distinct_tags() {
            let alice = TestNode::new(NodeConfig::testing(), "alice").await;
            let bobbi = TestNode::new(NodeConfig::testing(), "bobbi").await;
            let topic = TopicId::random();
            let data = b"shared-media-blob";

            let hash_alice = alice
                .blob_sync()
                .store_blob(topic, alice.device_id(), data.as_ref())
                .await
                .unwrap();
            let hash_bobbi = bobbi
                .blob_sync()
                .store_blob(topic, bobbi.device_id(), data.as_ref())
                .await
                .unwrap();

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
            let data = b"shared-media-blob";

            // Both nodes store the same blob under their own authorship.
            let hash = alice
                .blob_sync()
                .store_blob(topic, alice.device_id(), data.as_ref())
                .await
                .unwrap();
            alice
                .blob_sync()
                .store_blob(topic, bobbi.device_id(), data.as_ref())
                .await
                .unwrap();
            bobbi
                .blob_sync()
                .store_blob(topic, alice.device_id(), data.as_ref())
                .await
                .unwrap();
            bobbi
                .blob_sync()
                .store_blob(topic, bobbi.device_id(), data.as_ref())
                .await
                .unwrap();

            // Before deletion: two tags each.
            assert_eq!(tag_count_for_hash(&alice.blobs(), hash).await, 2);
            assert_eq!(tag_count_for_hash(&bobbi.blobs(), hash).await, 2);

            // Delete alice's authorship tag on both nodes.
            alice
                .blob_sync()
                .delete_blobs(topic, alice.device_id(), [hash])
                .await;
            bobbi
                .blob_sync()
                .delete_blobs(topic, alice.device_id(), [hash])
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
            let data = b"gc-target-blob";

            let hash = alice
                .blob_sync()
                .store_blob(topic, alice.device_id(), data.as_ref())
                .await
                .unwrap();
            alice
                .blob_sync()
                .store_blob(topic, bobbi.device_id(), data.as_ref())
                .await
                .unwrap();
            bobbi
                .blob_sync()
                .store_blob(topic, alice.device_id(), data.as_ref())
                .await
                .unwrap();
            bobbi
                .blob_sync()
                .store_blob(topic, bobbi.device_id(), data.as_ref())
                .await
                .unwrap();

            // Delete all tags on both nodes.
            for node in [&alice, &bobbi] {
                node.blob_sync()
                    .delete_blobs(topic, alice.device_id(), [hash])
                    .await;
                node.blob_sync()
                    .delete_blobs(topic, bobbi.device_id(), [hash])
                    .await;
            }

            // No pinning tags remain — blob is eligible for GC on the next cycle.
            assert_eq!(tag_count_for_hash(&alice.blobs(), hash).await, 0);
            assert_eq!(tag_count_for_hash(&bobbi.blobs(), hash).await, 0);
        }
    }

    #[tokio::test]
    async fn evicts_a_hash_after_max_failures_and_keeps_others() {
        let pool = BlobFetchPool::default();
        let dead = hash(1);
        let live = hash(2);
        pool.add(TopicId::random(), dead).await;
        pool.add(TopicId::random(), live).await;

        for _ in 0..MAX_FETCH_FAILURES {
            pool.on_failure(&(TopicId::random(), dead)).await;
        }

        let remaining = pool.stack.lock().await;
        assert!(remaining.iter().all(|(_, h)| *h == live));
        assert_eq!(remaining.len(), 1);
    }

    #[tokio::test]
    async fn evicts_every_entry_sharing_an_evicted_hash() {
        let pool = BlobFetchPool::default();
        let h = hash(7);
        pool.add(TopicId::random(), h).await;
        pool.add(TopicId::random(), h).await;

        for _ in 0..MAX_FETCH_FAILURES {
            pool.on_failure(&(TopicId::random(), h)).await;
        }
        assert!(pool.is_empty().await);
    }

    #[tokio::test]
    async fn re_adding_resets_the_failure_count() {
        let pool = BlobFetchPool::default();
        let h = hash(3);
        pool.add(TopicId::random(), h).await;

        for _ in 0..(MAX_FETCH_FAILURES - 1) {
            pool.on_failure(&(TopicId::random(), h)).await;
        }
        pool.add(TopicId::random(), h).await;
        // The reset means one more failure must not evict it.
        pool.on_failure(&(TopicId::random(), h)).await;
        assert!(!pool.is_empty().await);
    }
}
