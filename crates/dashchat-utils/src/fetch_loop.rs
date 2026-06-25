use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::time::Duration;

use tokio::time::Instant;

use tokio::task::JoinSet;

#[derive(Clone, Debug)]
pub struct FetchConfig {
    pub concurrency: usize,
    pub attempt_timeout: Duration,
    pub pass_interval: Duration,
    /// Minimum time between successive attempts of the same item.
    /// Setting this prevents early-wake notifications from
    /// causing tight retry loops when items fail fast.
    pub retry_cooldown: Duration,
}

impl Default for FetchConfig {
    fn default() -> Self {
        Self {
            concurrency: 4,
            attempt_timeout: Duration::from_secs(60),
            pass_interval: Duration::from_secs(60),
            retry_cooldown: Duration::from_secs(30),
        }
    }
}

/// A pool of work items the fetch loop drains. Implementors own the storage and
/// the wake signal; the loop is otherwise generic.
#[async_trait::async_trait]
pub trait FetchPool: Clone + Send + Sync + 'static {
    type Item: Clone + Send + 'static;
    type Key: Copy + Eq + Hash + Send;

    /// The per-pass dedup key for an item (the loop never re-attempts an item
    /// whose key it already tried this pass).
    fn key(item: &Self::Item) -> Self::Key;
    async fn is_empty(&self) -> bool;
    /// The next item whose key is not in `tried`, or `None` if all are tried.
    async fn next_untried(&self, tried: &HashSet<Self::Key>) -> Option<Self::Item>;
    async fn remove(&self, item: &Self::Item);
    /// Called after each failed fetch attempt for an item. The default is a
    /// no-op; implementors may use this to track failure counts and evict.
    async fn on_failure(&self, _item: &Self::Item) {}
    /// Resolves when an item is added, so the loop can wake early.
    async fn wait_for_add(&self);
}

/// Drain `pool` until cancelled. Each pass walks the stack with up to
/// `config.concurrency` fetches in flight, giving each item `attempt_timeout`.
/// An item for which `fetch` returns `true` is removed. With items still
/// outstanding the loop waits up to `pass_interval` since the pass began before
/// retrying, but a newly added item wakes it early; with an empty pool it parks.
/// A per-item `retry_cooldown` (defaulting to `pass_interval`) prevents early
/// wakes from retrying recently-failed items in a tight loop.
pub async fn fetch_loop<P, F, Fut>(pool: P, config: FetchConfig, fetch: F)
where
    P: FetchPool,
    F: Fn(P::Item, Duration) -> Fut + Clone + Send + 'static,
    Fut: std::future::Future<Output = bool> + Send + 'static,
{
    let concurrency = config.concurrency.max(1);
    let cooldown = config.retry_cooldown;
    let mut last_tried: HashMap<P::Key, Instant> = HashMap::new();
    loop {
        let pass_start = Instant::now();
        run_fetch_pass(
            &pool,
            concurrency,
            config.attempt_timeout,
            &fetch,
            &mut last_tried,
            cooldown,
        )
        .await;

        if pool.is_empty().await {
            pool.wait_for_add().await;
            continue;
        }
        let elapsed = pass_start.elapsed();
        if elapsed < config.pass_interval {
            tokio::select! {
                _ = tokio::time::sleep(config.pass_interval - elapsed) => {}
                _ = pool.wait_for_add() => {}
            }
        }
    }
}

async fn run_fetch_pass<P, F, Fut>(
    pool: &P,
    concurrency: usize,
    attempt_timeout: Duration,
    fetch: &F,
    last_tried: &mut HashMap<P::Key, Instant>,
    cooldown: Duration,
) where
    P: FetchPool,
    F: Fn(P::Item, Duration) -> Fut + Clone + Send + 'static,
    Fut: std::future::Future<Output = bool> + Send + 'static,
{
    let now = Instant::now();
    last_tried.retain(|_, t| now.duration_since(*t) < cooldown);
    let mut tried: HashSet<P::Key> = last_tried.keys().copied().collect();

    let mut in_flight: JoinSet<(P::Item, bool)> = JoinSet::new();
    loop {
        while in_flight.len() < concurrency {
            let Some(item) = pool.next_untried(&tried).await else {
                break;
            };
            let key = P::key(&item);
            tried.insert(key);
            last_tried.insert(key, Instant::now());
            let fetch = fetch.clone();
            in_flight.spawn(async move {
                let succeeded = fetch(item.clone(), attempt_timeout).await;
                (item, succeeded)
            });
        }
        let Some(joined) = in_flight.join_next().await else {
            break;
        };
        if let Ok((item, succeeded)) = joined {
            if succeeded {
                pool.remove(&item).await;
                last_tried.remove(&P::key(&item));
            } else {
                pool.on_failure(&item).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use tokio::sync::{Mutex, Notify, Semaphore};

    /// Minimal FetchPool whose items are `u8` ids (also their own key).
    #[derive(Clone, Default)]
    struct TestPool {
        items: Arc<Mutex<Vec<u8>>>,
        added: Arc<Notify>,
    }
    impl TestPool {
        async fn add(&self, n: u8) {
            self.items.lock().await.push(n);
            self.added.notify_one();
        }
        async fn len(&self) -> usize {
            self.items.lock().await.len()
        }
    }
    #[async_trait::async_trait]
    impl FetchPool for TestPool {
        type Item = u8;
        type Key = u8;
        fn key(item: &u8) -> u8 {
            *item
        }
        async fn is_empty(&self) -> bool {
            self.items.lock().await.is_empty()
        }
        async fn next_untried(&self, tried: &HashSet<u8>) -> Option<u8> {
            self.items
                .lock()
                .await
                .iter()
                .rev()
                .find(|n| !tried.contains(*n))
                .copied()
        }
        async fn remove(&self, item: &u8) {
            self.items.lock().await.retain(|n| n != item);
        }
        async fn wait_for_add(&self) {
            self.added.notified().await;
        }
    }

    fn config() -> FetchConfig {
        FetchConfig {
            concurrency: 2,
            attempt_timeout: Duration::from_secs(5),
            pass_interval: Duration::from_secs(60),
            retry_cooldown: Duration::from_secs(60),
        }
    }

    async fn settle() {
        for _ in 0..50 {
            tokio::task::yield_now().await;
        }
    }

    #[tokio::test(start_paused = true)]
    async fn empty_pool_parks_until_an_item_is_added() {
        let pool = TestPool::default();
        let calls = Arc::new(Mutex::new(Vec::new()));
        let handle = {
            let calls = calls.clone();
            tokio::spawn(fetch_loop(pool.clone(), config(), move |n, _t| {
                let calls = calls.clone();
                async move {
                    calls.lock().await.push(n);
                    true
                }
            }))
        };
        settle().await;
        assert!(calls.lock().await.is_empty());
        pool.add(1).await;
        settle().await;
        assert_eq!(calls.lock().await.as_slice(), &[1]);
        assert!(pool.is_empty().await);
        handle.abort();
    }

    #[tokio::test(start_paused = true)]
    async fn one_pass_drains_all_succeeding_items() {
        let pool = TestPool::default();
        for n in 1..=3 {
            pool.add(n).await;
        }
        let calls = Arc::new(Mutex::new(Vec::new()));
        let handle = {
            let calls = calls.clone();
            tokio::spawn(fetch_loop(pool.clone(), config(), move |n, _t| {
                let calls = calls.clone();
                async move {
                    calls.lock().await.push(n);
                    true
                }
            }))
        };
        settle().await;
        assert!(pool.is_empty().await);
        let fetched: HashSet<u8> = calls.lock().await.iter().copied().collect();
        assert_eq!(fetched, HashSet::from([1, 2, 3]));
        handle.abort();
    }

    #[tokio::test(start_paused = true)]
    async fn failing_item_is_retried_about_one_interval_later() {
        let pool = TestPool::default();
        let times = Arc::new(Mutex::new(Vec::new()));
        let start = tokio::time::Instant::now();
        let handle = {
            let times = times.clone();
            tokio::spawn(fetch_loop(pool.clone(), config(), move |_n, _t| {
                let times = times.clone();
                async move {
                    times.lock().await.push(start.elapsed());
                    false
                }
            }))
        };
        tokio::time::sleep(Duration::from_secs(1)).await;
        pool.add(1).await;
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
    async fn adding_an_item_wakes_the_loop_before_the_interval() {
        let pool = TestPool::default();
        pool.add(1).await;
        let times = Arc::new(Mutex::new(Vec::new()));
        let start = tokio::time::Instant::now();
        let handle = {
            let times = times.clone();
            tokio::spawn(fetch_loop(pool.clone(), config(), move |n, _t| {
                let times = times.clone();
                async move {
                    times.lock().await.push((n, start.elapsed()));
                    false
                }
            }))
        };
        tokio::time::sleep(Duration::from_secs(5)).await;
        pool.add(2).await;
        tokio::time::sleep(Duration::from_secs(1)).await;
        let times = times.lock().await.clone();
        let woke_early = times
            .iter()
            .any(|(n, at)| *n == 2 && *at < Duration::from_secs(30));
        assert!(woke_early, "expected item 2 fetched early, got {times:?}");
        handle.abort();
    }

    #[tokio::test(start_paused = true)]
    async fn early_wake_does_not_retry_recently_failed_item() {
        let pool = TestPool::default();
        pool.add(1).await;
        let calls: Arc<Mutex<Vec<(u8, Duration)>>> = Arc::new(Mutex::new(Vec::new()));
        let start = tokio::time::Instant::now();
        let handle = {
            let calls = calls.clone();
            tokio::spawn(fetch_loop(pool.clone(), config(), move |n, _t| {
                let calls = calls.clone();
                async move {
                    calls.lock().await.push((n, start.elapsed()));
                    false
                }
            }))
        };
        // Item 1 fails fast; at t=5s add item 2 to wake the loop early.
        tokio::time::sleep(Duration::from_secs(5)).await;
        pool.add(2).await;
        tokio::time::sleep(Duration::from_secs(10)).await;
        let calls = calls.lock().await.clone();
        // Item 1 should only have been attempted once (at ~t=0), not retried at ~t=5.
        let item1_calls: Vec<_> = calls.iter().filter(|(n, _)| *n == 1).collect();
        assert_eq!(
            item1_calls.len(),
            1,
            "item 1 was retried during cooldown: {calls:?}"
        );
        handle.abort();
    }

    #[tokio::test(start_paused = true)]
    async fn never_exceeds_the_concurrency_limit() {
        let pool = TestPool::default();
        for n in 1..=4 {
            pool.add(n).await;
        }
        let gate = Arc::new(Semaphore::new(0));
        let in_flight = Arc::new(AtomicUsize::new(0));
        let max_in_flight = Arc::new(AtomicUsize::new(0));
        let handle = {
            let gate = gate.clone();
            let in_flight = in_flight.clone();
            let max_in_flight = max_in_flight.clone();
            tokio::spawn(fetch_loop(pool.clone(), config(), move |_n, _t| {
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
        assert_eq!(pool.len().await, 4);
        gate.add_permits(2);
        settle().await;
        assert_eq!(max_in_flight.load(Ordering::SeqCst), 2);
        assert_eq!(pool.len().await, 2);
        gate.add_permits(2);
        settle().await;
        assert_eq!(max_in_flight.load(Ordering::SeqCst), 2);
        assert!(pool.is_empty().await);
        handle.abort();
    }
}
