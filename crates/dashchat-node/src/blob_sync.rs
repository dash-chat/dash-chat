use derive_more::derive::Constructor;
use futures::Stream;
use iroh_blobs::protocol::GetRequest;
use mailbox_client::manager::Mailboxes;
use p2panda::NodeId;
use p2panda::operation::{LogId, Operation};
use std::{
    collections::HashSet,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{
    sync::{Mutex, Notify},
    task::{JoinHandle, JoinSet},
};
use tokio_stream::StreamExt;

use crate::{AsBody, ChatPayload, Payload, mailbox::MailboxOperation, stores::OpStore};

/// Tuning parameters for the background blob fetch loop.
#[derive(Clone, Debug)]
pub struct BlobFetchConfig {
    /// Number of blob downloads attempted concurrently within a pass.
    pub concurrency: usize,
    /// How long a single blob download is given before the loop moves on.
    pub attempt_timeout: Duration,
    /// Minimum delay between passes over the fetch stack when items remain.
    pub pass_interval: Duration,
}

impl Default for BlobFetchConfig {
    fn default() -> Self {
        Self {
            concurrency: 4,
            attempt_timeout: Duration::from_secs(30),
            pass_interval: Duration::from_secs(60),
        }
    }
}

#[derive(Clone)]
pub struct BlobSync {
    pub blobs: iroh_blobs::BlobsProtocol,
    pub fetch_pool: BlobFetchPool,
    pub sources: MixedSourceLookup,
    endpoint: p2panda::Endpoint,
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
        endpoint.accept(iroh_blobs::ALPN, blobs.clone()).await?;

        Ok(Self {
            blobs,
            fetch_pool: blob_fetch,
            sources,
            endpoint,
        })
    }

    /// Spawn the background loop that drains the fetch pool, returning a handle
    /// that cancels the loop when aborted.
    ///
    /// Each pass walks the whole stack from the top with up to `concurrency`
    /// downloads in flight, giving every blob until `attempt_timeout` to arrive
    /// before moving on. Successfully stored blobs are removed from the stack.
    /// When a pass finishes with items still outstanding, the loop waits until
    /// `pass_interval` has elapsed since the pass began before trying again, but
    /// a newly added hash wakes it early so fresh blobs are attempted at once.
    pub fn spawn_fetch_loop(&self, config: BlobFetchConfig) -> JoinHandle<()> {
        let this = self.clone();
        let pool = self.fetch_pool.clone();
        tokio::spawn(fetch_loop(
            pool,
            config,
            move |log_id, hash, attempt_timeout| {
                let this = this.clone();
                async move { this.try_fetch(log_id, hash, attempt_timeout).await }
            },
        ))
    }

    /// Attempt to fetch a single blob, returning `true` when it is present in
    /// the local store afterwards (already cached or newly downloaded).
    async fn try_fetch(
        &self,
        log_id: LogId,
        hash: iroh_blobs::Hash,
        attempt_timeout: Duration,
    ) -> bool {
        if self.blobs.has(hash).await.unwrap_or(false) {
            return true;
        }

        let sources = match self.sources.sources(log_id).await {
            Ok(sources) => sources,
            Err(err) => {
                tracing::warn!(%hash, ?err, "blob source lookup failed");
                return false;
            }
        };

        if sources.is_empty() {
            return false;
        }

        match tokio::time::timeout(attempt_timeout, self.download_from_any(hash, sources)).await {
            Ok(downloaded) => downloaded,
            Err(_) => {
                tracing::warn!(%hash, "blob download timed out");
                false
            }
        }
    }

    /// Try each source in turn until one serves the blob.
    async fn download_from_any(&self, hash: iroh_blobs::Hash, sources: Vec<NodeId>) -> bool {
        for source in sources {
            match self.download_from(source, hash).await {
                Ok(()) => return true,
                Err(err) => {
                    tracing::debug!(%hash, %source, ?err, "blob download from source failed");
                }
            }
        }
        false
    }

    /// Connect to a single source and fetch the blob into the local store.
    ///
    /// The connection is opened through the p2panda endpoint so that the ALPN
    /// is mixed with our network id the same way the serving side registered it
    /// — iroh-blobs' own `Downloader` dials the bare ALPN and would be rejected.
    async fn download_from(&self, source: NodeId, hash: iroh_blobs::Hash) -> anyhow::Result<()> {
        let conn = self.endpoint.connect(source, iroh_blobs::ALPN).await?;
        self.blobs
            .remote()
            .execute_get(conn, GetRequest::all(hash))
            .complete()
            .await?;
        Ok(())
    }
}

/// Drain the fetch pool until cancelled.
///
/// Each pass walks the whole stack from the top with up to
/// `config.concurrency` downloads in flight, giving every blob until
/// `config.attempt_timeout` to arrive before moving on. A blob for which
/// `fetch` returns `true` is removed from the stack. When a pass finishes with
/// items still outstanding, the loop waits until `config.pass_interval` has
/// elapsed since the pass began before trying again, but a newly added hash
/// wakes it early. With an empty stack the loop parks until something is added.
async fn fetch_loop<F, Fut>(pool: BlobFetchPool, config: BlobFetchConfig, fetch: F)
where
    F: Fn(LogId, iroh_blobs::Hash, Duration) -> Fut + Clone + Send + 'static,
    Fut: std::future::Future<Output = bool> + Send + 'static,
{
    let concurrency = config.concurrency.max(1);
    loop {
        let pass_start = Instant::now();
        run_fetch_pass(&pool, concurrency, config.attempt_timeout, &fetch).await;

        if pool.is_empty().await {
            pool.added.notified().await;
            continue;
        }

        let elapsed = pass_start.elapsed();
        if elapsed < config.pass_interval {
            tokio::select! {
                _ = tokio::time::sleep(config.pass_interval - elapsed) => {}
                _ = pool.added.notified() => {}
            }
        }
    }
}

async fn run_fetch_pass<F, Fut>(
    pool: &BlobFetchPool,
    concurrency: usize,
    attempt_timeout: Duration,
    fetch: &F,
) where
    F: Fn(LogId, iroh_blobs::Hash, Duration) -> Fut + Clone + Send + 'static,
    Fut: std::future::Future<Output = bool> + Send + 'static,
{
    let mut tried: HashSet<iroh_blobs::Hash> = HashSet::new();
    let mut in_flight: JoinSet<Option<(LogId, iroh_blobs::Hash)>> = JoinSet::new();

    loop {
        while in_flight.len() < concurrency {
            let Some((log_id, hash)) = pool.next_untried(&tried).await else {
                break;
            };
            tried.insert(hash);
            let fetch = fetch.clone();
            in_flight.spawn(async move {
                fetch(log_id, hash, attempt_timeout)
                    .await
                    .then_some((log_id, hash))
            });
        }

        let Some(joined) = in_flight.join_next().await else {
            break;
        };
        if let Ok(Some((log_id, hash))) = joined {
            pool.remove(log_id, hash).await;
        }
    }
}

#[derive(Clone, Default)]
pub struct BlobFetchPool {
    stack: Arc<Mutex<Vec<(LogId, iroh_blobs::Hash)>>>,
    added: Arc<Notify>,
}

impl BlobFetchPool {
    pub async fn add(&self, log_id: LogId, hash: iroh_blobs::Hash) {
        self.stack.lock().await.push((log_id, hash));
        self.added.notify_one();
    }

    async fn is_empty(&self) -> bool {
        self.stack.lock().await.is_empty()
    }

    /// Return the topmost (most recently added) entry whose hash has not yet
    /// been attempted this pass, so freshly added blobs are tried first.
    async fn next_untried(
        &self,
        tried: &HashSet<iroh_blobs::Hash>,
    ) -> Option<(LogId, iroh_blobs::Hash)> {
        let stack = self.stack.lock().await;
        stack
            .iter()
            .rev()
            .find(|(_, hash)| !tried.contains(hash))
            .copied()
    }

    async fn remove(&self, log_id: LogId, hash: iroh_blobs::Hash) {
        self.stack
            .lock()
            .await
            .retain(|entry| entry != &(log_id, hash));
    }

    // TODO: can we just have a p2panda stream of all past and future operations?
    pub async fn from_ops(
        ops: impl Stream<Item = Result<Operation, anyhow::Error>> + '_,
    ) -> anyhow::Result<Self> {
        let store = Self::default();
        let mut s = store.stack.lock().await;
        tokio::pin!(ops);
        while let Some(op) = ops.try_next().await? {
            let Some(body) = op.body else {
                continue;
            };
            let payload = Payload::try_from_body(&body)?;
            match payload {
                Payload::Chat(ChatPayload::Message(m)) => {
                    if let Some(media) = m.media_meta() {
                        for item in media {
                            s.push((op.header.extensions.log_id, item.hash));
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
}

impl MixedSourceLookup {
    pub async fn sources(&self, log_id: LogId) -> anyhow::Result<Vec<NodeId>> {
        let sources = self
            .op_store
            .get_authors(log_id)
            .await?
            .into_iter()
            .map(|author| *author)
            .collect();
        // sources.extend(self.mailboxes.get_sources(log_id).await?);
        Ok(sources)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::sync::Semaphore;

    fn config() -> BlobFetchConfig {
        BlobFetchConfig {
            concurrency: 2,
            attempt_timeout: Duration::from_secs(5),
            pass_interval: Duration::from_secs(60),
        }
    }

    fn log_id() -> LogId {
        LogId::from_topic(crate::topic::TopicId::from([0u8; 32]))
    }

    fn hash(n: u8) -> iroh_blobs::Hash {
        iroh_blobs::Hash::new([n; 32])
    }

    async fn settle() {
        for _ in 0..50 {
            tokio::task::yield_now().await;
        }
    }

    async fn pool_len(pool: &BlobFetchPool) -> usize {
        pool.stack.lock().await.len()
    }

    #[tokio::test(start_paused = true)]
    async fn empty_pool_parks_until_a_hash_is_added() {
        let pool = BlobFetchPool::default();
        let calls = Arc::new(Mutex::new(Vec::new()));

        let handle = {
            let calls = calls.clone();
            tokio::spawn(fetch_loop(pool.clone(), config(), move |_l, h, _t| {
                let calls = calls.clone();
                async move {
                    calls.lock().await.push(h);
                    true
                }
            }))
        };

        settle().await;
        assert!(calls.lock().await.is_empty());

        pool.add(log_id(), hash(1)).await;
        settle().await;

        assert_eq!(calls.lock().await.as_slice(), &[hash(1)]);
        assert!(pool.is_empty().await);
        handle.abort();
    }

    #[tokio::test(start_paused = true)]
    async fn one_pass_drains_all_succeeding_items() {
        let pool = BlobFetchPool::default();
        for n in 1..=3 {
            pool.add(log_id(), hash(n)).await;
        }
        let calls = Arc::new(Mutex::new(Vec::new()));

        let handle = {
            let calls = calls.clone();
            tokio::spawn(fetch_loop(pool.clone(), config(), move |_l, h, _t| {
                let calls = calls.clone();
                async move {
                    calls.lock().await.push(h);
                    true
                }
            }))
        };

        settle().await;

        assert!(pool.is_empty().await);
        let fetched: HashSet<_> = calls.lock().await.iter().copied().collect();
        assert_eq!(fetched, HashSet::from([hash(1), hash(2), hash(3)]));
        handle.abort();
    }

    #[tokio::test(start_paused = true)]
    async fn failing_item_is_retried_about_one_interval_later() {
        let pool = BlobFetchPool::default();
        let times = Arc::new(Mutex::new(Vec::new()));
        let start = tokio::time::Instant::now();

        let handle = {
            let times = times.clone();
            tokio::spawn(fetch_loop(pool.clone(), config(), move |_l, _h, _t| {
                let times = times.clone();
                async move {
                    times.lock().await.push(start.elapsed());
                    false
                }
            }))
        };

        // Let the loop park on the empty pool, then add an item so the wake is
        // driven solely by the add (no stale notify permit skewing the timing).
        tokio::time::sleep(Duration::from_secs(1)).await;
        pool.add(log_id(), hash(1)).await;
        tokio::time::sleep(Duration::from_secs(61)).await;

        let times = times.lock().await.clone();
        assert_eq!(times.len(), 2);
        let gap = times[1] - times[0];
        assert!(
            gap >= Duration::from_secs(59) && gap <= Duration::from_secs(61),
            "expected ~60s between passes, got {gap:?}"
        );
        handle.abort();
    }

    #[tokio::test(start_paused = true)]
    async fn adding_a_hash_wakes_the_loop_before_the_interval() {
        let pool = BlobFetchPool::default();
        pool.add(log_id(), hash(1)).await;
        let times = Arc::new(Mutex::new(Vec::new()));
        let start = tokio::time::Instant::now();

        let handle = {
            let times = times.clone();
            tokio::spawn(fetch_loop(pool.clone(), config(), move |_l, h, _t| {
                let times = times.clone();
                async move {
                    times.lock().await.push((h, start.elapsed()));
                    false
                }
            }))
        };

        tokio::time::sleep(Duration::from_secs(5)).await;
        pool.add(log_id(), hash(2)).await;
        tokio::time::sleep(Duration::from_secs(1)).await;

        let times = times.lock().await.clone();
        let woke_early = times
            .iter()
            .any(|(h, at)| *h == hash(2) && *at < Duration::from_secs(30));
        assert!(
            woke_early,
            "expected hash(2) to be fetched early, got {times:?}"
        );
        handle.abort();
    }

    #[tokio::test(start_paused = true)]
    async fn never_exceeds_the_concurrency_limit() {
        let pool = BlobFetchPool::default();
        for n in 1..=4 {
            pool.add(log_id(), hash(n)).await;
        }

        let gate = Arc::new(Semaphore::new(0));
        let in_flight = Arc::new(AtomicUsize::new(0));
        let max_in_flight = Arc::new(AtomicUsize::new(0));

        let handle = {
            let gate = gate.clone();
            let in_flight = in_flight.clone();
            let max_in_flight = max_in_flight.clone();
            tokio::spawn(fetch_loop(pool.clone(), config(), move |_l, _h, _t| {
                let gate = gate.clone();
                let in_flight = in_flight.clone();
                let max_in_flight = max_in_flight.clone();
                async move {
                    let now = in_flight.fetch_add(1, Ordering::SeqCst) + 1;
                    max_in_flight.fetch_max(now, Ordering::SeqCst);
                    gate.acquire().await.unwrap().forget();
                    in_flight.fetch_sub(1, Ordering::SeqCst);
                    true
                }
            }))
        };

        settle().await;
        assert_eq!(in_flight.load(Ordering::SeqCst), 2);
        assert_eq!(max_in_flight.load(Ordering::SeqCst), 2);
        assert_eq!(pool_len(&pool).await, 4);

        gate.add_permits(2);
        settle().await;
        assert_eq!(max_in_flight.load(Ordering::SeqCst), 2);
        assert_eq!(pool_len(&pool).await, 2);

        gate.add_permits(2);
        settle().await;
        assert_eq!(max_in_flight.load(Ordering::SeqCst), 2);
        assert!(pool.is_empty().await);
        handle.abort();
    }
}
