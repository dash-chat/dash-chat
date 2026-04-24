use std::fmt::Display;
use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Notify;

/// A task that can be triggered any number of times but only runs one
/// retry-with-backoff cycle at a time. Starts listening for triggers
/// immediately on construction.
///
/// - Calling [`trigger`] while the task is idle starts a new retry cycle.
/// - Calling [`trigger`] while a cycle is already running is a no-op.
/// - The closure is called fresh on every attempt, so it can read external
///   state (e.g. a settings file) to decide what to do.
///
/// [`trigger`]: SingletonTaskWithRetries::trigger
pub struct SingletonTaskWithRetries {
    inner: Arc<Inner>,
}

struct Inner {
    notify: Notify,
    running: AtomicBool,
}

impl SingletonTaskWithRetries {
    pub fn new<T, E, F, Fut>(
        label: impl Into<String>,
        max_attempts: Option<u32>,
        initial_delay: Duration,
        max_delay: Duration,
        task: F,
    ) -> Self
    where
        T: Send + 'static,
        E: Display + Send + 'static,
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T, E>> + Send,
    {
        let inner = Arc::new(Inner {
            notify: Notify::new(),
            running: AtomicBool::new(false),
        });

        let state = inner.clone();
        let label = label.into();

        tokio::spawn(async move {
            loop {
                state.notify.notified().await;
                state.running.store(true, Ordering::SeqCst);

                let mut delay = initial_delay;
                let mut attempt: u32 = 0;

                loop {
                    attempt = attempt.saturating_add(1);

                    match task().await {
                        Ok(_) => {
                            tracing::info!("Successfully completed task: {label}.");
                            break;
                        }
                        Err(e) => {
                            let exhausted = max_attempts.is_some_and(|max| attempt >= max);

                            if exhausted {
                                tracing::warn!(
                                    "{label} failed after {attempt} attempts, giving up: {e}",
                                );
                                break;
                            }

                            match max_attempts {
                                Some(max) => tracing::warn!(
                                    "{label} failed (attempt {attempt}/{max}): {e}. \
                                     Retrying in {}s.",
                                    delay.as_secs(),
                                ),
                                None => tracing::warn!(
                                    "{label} failed (attempt {attempt}): {e}. \
                                     Retrying in {}s.",
                                    delay.as_secs(),
                                ),
                            }

                            tokio::time::sleep(delay).await;
                            delay = (delay * 2).min(max_delay);
                        }
                    }
                }

                state.running.store(false, Ordering::SeqCst);
            }
        });

        Self { inner }
    }

    /// Signal the task to run. If it is already running, this is a no-op.
    pub fn trigger(&self) {
        if !self.inner.running.load(Ordering::SeqCst) {
            self.inner.notify.notify_one();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::atomic::AtomicU32;

    #[tokio::test]
    async fn succeeds_on_first_attempt() {
        let call_count = Arc::new(AtomicU32::new(0));
        let cc = call_count.clone();

        let task = SingletonTaskWithRetries::new(
            "test",
            None,
            Duration::from_millis(1),
            Duration::from_millis(10),
            move || {
                cc.fetch_add(1, Ordering::SeqCst);
                async { Ok::<(), &str>(()) }
            },
        );

        task.trigger();
        tokio::time::sleep(Duration::from_millis(50)).await;

        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn retries_until_success() {
        let call_count = Arc::new(AtomicU32::new(0));
        let cc = call_count.clone();

        let task = SingletonTaskWithRetries::new(
            "test",
            None,
            Duration::from_millis(1),
            Duration::from_millis(5),
            move || {
                let attempt = cc.fetch_add(1, Ordering::SeqCst) + 1;
                async move {
                    if attempt < 3 {
                        Err("not yet")
                    } else {
                        Ok(())
                    }
                }
            },
        );

        task.trigger();
        tokio::time::sleep(Duration::from_millis(100)).await;

        assert_eq!(call_count.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn gives_up_after_max_attempts() {
        let call_count = Arc::new(AtomicU32::new(0));
        let cc = call_count.clone();

        let task = SingletonTaskWithRetries::new(
            "test",
            Some(3),
            Duration::from_millis(1),
            Duration::from_millis(5),
            move || {
                cc.fetch_add(1, Ordering::SeqCst);
                async { Err::<(), _>("always fails") }
            },
        );

        task.trigger();
        tokio::time::sleep(Duration::from_millis(100)).await;

        assert_eq!(call_count.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn trigger_during_run_is_noop() {
        let call_count = Arc::new(AtomicU32::new(0));
        let cc = call_count.clone();

        let task = SingletonTaskWithRetries::new(
            "test",
            None,
            Duration::from_millis(50),
            Duration::from_millis(50),
            move || {
                let attempt = cc.fetch_add(1, Ordering::SeqCst) + 1;
                async move {
                    if attempt < 2 {
                        Err("not yet")
                    } else {
                        Ok(())
                    }
                }
            },
        );

        task.trigger();
        // Trigger again while it's in the backoff sleep — should be ignored
        tokio::time::sleep(Duration::from_millis(10)).await;
        task.trigger();
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Only 2 calls: the initial failure + the retry success. No extra run.
        assert_eq!(call_count.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn multiple_triggers_while_idle_coalesce_to_one_run() {
        let call_count = Arc::new(AtomicU32::new(0));
        let cc = call_count.clone();

        let task = SingletonTaskWithRetries::new(
            "test",
            None,
            Duration::from_millis(1),
            Duration::from_millis(5),
            move || {
                cc.fetch_add(1, Ordering::SeqCst);
                async { Ok::<(), &str>(()) }
            },
        );

        task.trigger();
        task.trigger();
        task.trigger();

        tokio::time::sleep(Duration::from_millis(100)).await;

        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn does_not_run_without_trigger() {
        let call_count = Arc::new(AtomicU32::new(0));
        let cc = call_count.clone();

        let _task = SingletonTaskWithRetries::new(
            "test",
            None,
            Duration::from_millis(1),
            Duration::from_millis(5),
            move || {
                cc.fetch_add(1, Ordering::SeqCst);
                async { Ok::<(), &str>(()) }
            },
        );

        tokio::time::sleep(Duration::from_millis(50)).await;

        assert_eq!(call_count.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn closure_reads_external_state_each_attempt() {
        let external_flag = Arc::new(AtomicU32::new(0));
        let observed_values = Arc::new(std::sync::Mutex::new(Vec::new()));

        let flag = external_flag.clone();
        let obs = observed_values.clone();

        let task = SingletonTaskWithRetries::new(
            "test",
            None,
            Duration::from_millis(1),
            Duration::from_millis(5),
            move || {
                let val = flag.load(Ordering::SeqCst);
                obs.lock().unwrap().push(val);
                async move {
                    if val < 2 {
                        Err("not ready")
                    } else {
                        Ok(())
                    }
                }
            },
        );

        task.trigger();
        tokio::time::sleep(Duration::from_millis(10)).await;
        external_flag.store(1, Ordering::SeqCst);
        tokio::time::sleep(Duration::from_millis(10)).await;
        external_flag.store(2, Ordering::SeqCst);
        tokio::time::sleep(Duration::from_millis(100)).await;

        let values = observed_values.lock().unwrap().clone();
        assert!(values.contains(&0));
        assert!(values.contains(&2));
        assert_eq!(*values.last().unwrap(), 2);
    }

    #[tokio::test]
    async fn can_trigger_again_after_cycle_completes() {
        let call_count = Arc::new(AtomicU32::new(0));
        let cc = call_count.clone();

        let task = SingletonTaskWithRetries::new(
            "test",
            None,
            Duration::from_millis(1),
            Duration::from_millis(5),
            move || {
                cc.fetch_add(1, Ordering::SeqCst);
                async { Ok::<(), &str>(()) }
            },
        );

        task.trigger();
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(call_count.load(Ordering::SeqCst), 1);

        task.trigger();
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(call_count.load(Ordering::SeqCst), 2);
    }
}
