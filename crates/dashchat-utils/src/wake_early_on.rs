use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use backon::{Backoff, Retry, Sleeper};
use futures::future::BoxFuture;
use tokio::sync::Notify;

/// Lets a backon [`Retry`] act on a change at once instead of after its backoff.
pub trait WakeEarlyOn<B, T, E, Fut, FutureFn, RF, NF, AF>
where
    B: Backoff,
    Fut: Future<Output = Result<T, E>>,
    FutureFn: FnMut() -> Fut,
{
    /// End each delay between attempts early when `notify` fires.
    fn wake_early_on(
        self,
        notify: Arc<Notify>,
    ) -> Retry<B, T, E, Fut, FutureFn, SleepUntilNotified, RF, NF, AF>;
}

impl<B, T, E, Fut, FutureFn, SF, RF, NF, AF> WakeEarlyOn<B, T, E, Fut, FutureFn, RF, NF, AF>
    for Retry<B, T, E, Fut, FutureFn, SF, RF, NF, AF>
where
    B: Backoff,
    Fut: Future<Output = Result<T, E>>,
    FutureFn: FnMut() -> Fut,
    SF: backon::Sleeper,
    RF: FnMut(&E) -> bool,
    NF: FnMut(&E, Duration),
    AF: FnMut(&E, Option<Duration>) -> Option<Duration>,
{
    fn wake_early_on(
        self,
        notify: Arc<Notify>,
    ) -> Retry<B, T, E, Fut, FutureFn, SleepUntilNotified, RF, NF, AF> {
        self.sleep(SleepUntilNotified(notify))
    }
}

/// Sleeps for the delay, or until the `Notify` fires, whichever comes first.
pub struct SleepUntilNotified(Arc<Notify>);

impl Sleeper for SleepUntilNotified {
    type Sleep = BoxFuture<'static, ()>;

    fn sleep(&self, delay: Duration) -> Self::Sleep {
        let notify = self.0.clone();
        Box::pin(async move {
            tokio::select! {
                () = tokio::time::sleep(delay) => {}
                () = notify.notified() => {}
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU32, Ordering};

    use backon::{ExponentialBuilder, Retryable};

    use super::*;

    #[tokio::test]
    async fn a_notification_ends_the_backoff_early() {
        let notify = Arc::new(Notify::new());
        let attempts = Arc::new(AtomicU32::new(0));
        let notifier = notify.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            notifier.notify_one();
        });

        let started = tokio::time::Instant::now();
        let result = (|| {
            let attempt = attempts.fetch_add(1, Ordering::SeqCst) + 1;
            async move {
                if attempt == 1 {
                    anyhow::bail!("first attempt fails");
                }
                Ok(())
            }
        })
        .retry(ExponentialBuilder::new().with_min_delay(Duration::from_secs(10)))
        .wake_early_on(notify)
        .await;

        assert!(result.is_ok());
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
        assert!(started.elapsed() < Duration::from_secs(1));
    }
}
