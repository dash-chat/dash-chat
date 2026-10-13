//! SQLite pools whose writes can be suspended.
//!
//! iOS kills (`0xdead10cc`) an app it suspends while a write transaction is
//! open on a database in the App Group container; idle connections and reads
//! don't count. Suspending the gate parks every new connection request and
//! waits for the connections in use to come back.

use std::future::Future;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use tokio::sync::watch;

/// How many connections were still busy when [`SqliteGate::suspend`] gave up.
#[derive(Debug)]
pub struct StillInUseError(pub usize);

impl std::fmt::Display for StillInUseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} connections still in use", self.0)
    }
}

impl std::error::Error for StillInUseError {}

/// Clones share the same gate.
#[derive(Clone, Debug)]
pub struct SqliteGate {
    open: watch::Sender<bool>,
    parked_requests: Arc<AtomicUsize>,
    /// Weak, so the gate never keeps a pool its owner dropped.
    pools: Arc<Mutex<Vec<Weak<SqlitePool>>>>,
}

/// A pool connected through a [`SqliteGate`]. Hold it for as long as the pool
/// is used: the gate forgets the pool once every handle is dropped.
#[derive(Clone, Debug)]
pub struct GatedPool(Arc<SqlitePool>);

impl GatedPool {
    pub fn pool(&self) -> &SqlitePool {
        &self.0
    }
}

struct Parked<'a>(&'a AtomicUsize);

impl Drop for Parked<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

impl Default for SqliteGate {
    fn default() -> Self {
        Self::new()
    }
}

impl SqliteGate {
    pub fn new() -> Self {
        Self {
            open: watch::Sender::new(true),
            parked_requests: Arc::default(),
            pools: Arc::default(),
        }
    }

    pub async fn connect(
        &self,
        pool_options: SqlitePoolOptions,
        connect_options: SqliteConnectOptions,
    ) -> Result<GatedPool, sqlx::Error> {
        let pool = GatedPool(Arc::new(
            self.with_parking_hooks(pool_options)
                .connect_lazy_with(connect_options),
        ));
        self.pools.lock().unwrap().push(Arc::downgrade(&pool.0));
        pool.pool().acquire().await?;
        Ok(pool)
    }

    /// Parks new connection requests right away and returns the wait for the
    /// busy connections to come back. Not `async`, so suspend and resume take
    /// effect in call order even when the returned futures run out of order.
    /// Past `deadline` the gate stays closed anyway: reopening it would let new
    /// writes start just before the app is suspended.
    pub fn suspend(
        &self,
        deadline: Duration,
    ) -> impl Future<Output = Result<(), StillInUseError>> + Send + 'static {
        self.open.send_replace(false);
        let gate = self.clone();
        async move {
            let drained = tokio::time::timeout(deadline, async {
                while !gate.is_open() && gate.busy_connections() > 0 {
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
            })
            .await;
            match drained {
                Ok(()) => Ok(()),
                Err(_) => Err(StillInUseError(gate.busy_connections())),
            }
        }
    }

    pub fn resume(&self) {
        self.open.send_replace(true);
    }

    fn is_open(&self) -> bool {
        *self.open.borrow()
    }

    async fn wait_until_open(&self) {
        if self.is_open() {
            return;
        }
        self.parked_requests.fetch_add(1, Ordering::SeqCst);
        // Counted down on drop too, so a cancelled request doesn't stay counted.
        let _parked = Parked(&self.parked_requests);
        let mut open = self.open.subscribe();
        let _ = open.wait_for(|open| *open).await;
    }

    /// A parked request holds a connection but can't be inside a transaction,
    /// so it doesn't count.
    fn busy_connections(&self) -> usize {
        let mut pools = self.pools.lock().unwrap();
        pools.retain(|pool| pool.upgrade().is_some_and(|pool| !pool.is_closed()));
        let checked_out: usize = pools
            .iter()
            .filter_map(Weak::upgrade)
            .map(|pool| (pool.size() as usize).saturating_sub(pool.num_idle()))
            .sum();
        checked_out.saturating_sub(self.parked_requests.load(Ordering::SeqCst))
    }

    fn with_parking_hooks(&self, options: SqlitePoolOptions) -> SqlitePoolOptions {
        let before_acquire = self.clone();
        let after_connect = self.clone();
        // Only iOS suspends its gate, and a request parked there counts towards
        // the acquire timeout for as long as the app is in the background.
        #[cfg(target_os = "ios")]
        let options = options.acquire_timeout(Duration::from_secs(24 * 60 * 60));
        options
            .before_acquire(move |_, _| {
                let gate = before_acquire.clone();
                Box::pin(async move {
                    gate.wait_until_open().await;
                    Ok(true)
                })
            })
            .after_connect(move |_, _| {
                let gate = after_connect.clone();
                Box::pin(async move {
                    gate.wait_until_open().await;
                    Ok(())
                })
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn pool(gate: &SqliteGate) -> GatedPool {
        gate.connect(
            SqlitePoolOptions::new().max_connections(2),
            SqliteConnectOptions::new().in_memory(true),
        )
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn suspend_waits_for_a_busy_connection() {
        let gate = SqliteGate::new();
        let pool = pool(&gate).await;
        let connection = pool.pool().acquire().await.unwrap();

        let suspended = tokio::spawn(gate.suspend(Duration::from_millis(500)));
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(!suspended.is_finished());
        drop(connection);
        suspended.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn a_suspended_gate_parks_new_requests_until_resumed() {
        let gate = SqliteGate::new();
        let pool = pool(&gate).await;

        // A parked request holds no transaction, so it doesn't hold suspending up.
        let suspended = gate.suspend(Duration::from_millis(500));
        let waiting = tokio::spawn({
            let pool = pool.clone();
            async move { pool.pool().acquire().await.map(|_| ()) }
        });
        suspended.await.unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(!waiting.is_finished());

        gate.resume();
        waiting.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn suspend_gives_up_after_the_deadline_but_stays_suspended() {
        let gate = SqliteGate::new();
        let pool = pool(&gate).await;
        let _connection = pool.pool().acquire().await.unwrap();

        let err = gate.suspend(Duration::from_millis(50)).await.unwrap_err();
        assert_eq!(err.0, 1);
        let waiting = tokio::spawn({
            let pool = pool.clone();
            async move { pool.pool().acquire().await.map(|_| ()) }
        });
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(!waiting.is_finished());
        gate.resume();
        waiting.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn a_cancelled_request_does_not_stay_parked() {
        let gate = SqliteGate::new();
        let pool = pool(&gate).await;

        let suspended = gate.suspend(Duration::from_millis(500));
        let cancelled = tokio::spawn({
            let pool = pool.clone();
            async move { pool.pool().acquire().await.map(|_| ()) }
        });
        tokio::time::sleep(Duration::from_millis(50)).await;
        cancelled.abort();
        let _ = cancelled.await;
        suspended.await.unwrap();
        gate.resume();

        let connection = pool.pool().acquire().await.unwrap();
        // One busy connection, none parked: suspending has to wait for it.
        assert!(gate.suspend(Duration::from_millis(50)).await.is_err());
        drop(connection);
    }

    #[tokio::test]
    async fn the_gate_forgets_a_pool_whose_handles_are_dropped() {
        let gate = SqliteGate::new();
        let pool = pool(&gate).await;
        let weak = Arc::downgrade(&pool.0);

        drop(pool);
        assert!(weak.upgrade().is_none());
        gate.suspend(Duration::from_millis(50)).await.unwrap();
        assert!(gate.pools.lock().unwrap().is_empty());
    }
}
