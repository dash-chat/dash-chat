use std::fmt::Display;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Notify;

/// A task that can be triggered any number of times but only runs one
/// retry-with-backoff cycle at a time. Starts listening for triggers
/// immediately on construction.
///
/// - Calling [`trigger`] while the task is idle starts a new retry cycle.
/// - Calling [`trigger`] while a cycle is already running cancels the
///   current cycle and restarts from scratch (resetting attempts and delays).
/// - The closure is called fresh on every attempt, so it can read external
///   state (e.g. a settings file) to decide what to do.
///
/// [`trigger`]: SingletonTaskWithRetries::trigger
#[derive(Clone)]
pub struct SingletonTaskWithRetries {
    inner: Arc<Inner>,
}

struct Inner {
    notify: Notify,
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
        });

        let state = inner.clone();
        let label = label.into();

        tokio::spawn(async move {
            loop {
                state.notify.notified().await;

                let mut delay = initial_delay;
                let mut attempt: u32 = 0;
                let mut cancelled = false;

                loop {
                    attempt = attempt.saturating_add(1);

                    let notified = state.notify.notified();
                    tokio::pin!(notified);

                    let result = tokio::select! {
                        biased;
                        _ = &mut notified => { cancelled = true; break; }
                        result = task() => result,
                    };

                    match result {
                        Ok(_) => {
                            log::info!("Successfully completed task: {label}.");
                            break;
                        }
                        Err(e) => {
                            let exhausted = max_attempts.is_some_and(|max| attempt >= max);

                            if exhausted {
                                log::warn!(
                                    "{label} failed after {attempt} attempts, giving up: {e}",
                                );
                                break;
                            }

                            match max_attempts {
                                Some(max) => log::warn!(
                                    "{label} failed (attempt {attempt}/{max}): {e}. \
                                     Retrying in {}s.",
                                    delay.as_secs(),
                                ),
                                None => log::warn!(
                                    "{label} failed (attempt {attempt}): {e}. \
                                     Retrying in {}s.",
                                    delay.as_secs(),
                                ),
                            }

                            tokio::select! {
                                biased;
                                _ = notified => { cancelled = true; break; }
                                _ = tokio::time::sleep(delay) => {}
                            }
                            delay = (delay * 2).min(max_delay);
                        }
                    }
                }

                if cancelled {
                    log::info!("{label} cancelled by new trigger, restarting.");
                    // Re-queue so the outer notified().await returns immediately
                    state.notify.notify_one();
                }
            }
        });

        Self { inner }
    }

    /// Signal the task to run. If it is already running, the current cycle
    /// is cancelled and the task restarts from scratch.
    pub fn trigger(&self) {
        self.inner.notify.notify_one();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::atomic::{AtomicU32, Ordering};

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
    async fn trigger_during_backoff_cancels_and_restarts() {
        let call_count = Arc::new(AtomicU32::new(0));
        let cc = call_count.clone();

        let task = SingletonTaskWithRetries::new(
            "test",
            None,
            Duration::from_millis(500),
            Duration::from_millis(500),
            move || {
                let attempt = cc.fetch_add(1, Ordering::SeqCst) + 1;
                async move {
                    if attempt == 1 {
                        Err("fail first time")
                    } else {
                        Ok(())
                    }
                }
            },
        );

        task.trigger();
        // Wait for first attempt to fail and enter backoff sleep (500ms)
        tokio::time::sleep(Duration::from_millis(20)).await;
        // Re-trigger during backoff — should cancel and restart from scratch
        task.trigger();
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Attempt 1: fail (original trigger)
        // Cancelled during backoff, restarted immediately
        // Attempt 2: succeed (new cycle)
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
