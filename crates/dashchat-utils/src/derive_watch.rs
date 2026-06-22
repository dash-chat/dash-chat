use std::future::Future;

use tokio::sync::watch;

/// Derive a new watch channel from `source` by applying an async transform to
/// the current value and every subsequent change.
pub async fn derive_watch<A, B, F, Fut>(
    mut source: watch::Receiver<A>,
    mut map: F,
) -> watch::Receiver<B>
where
    A: Clone + Send + Sync + 'static,
    B: Clone + Send + Sync + 'static,
    F: FnMut(A) -> Fut + Send + 'static,
    Fut: Future<Output = B> + Send,
{
    let arg = source.borrow().clone();
    let (tx, rx) = watch::channel(map(arg).await);
    tokio::spawn(async move {
        while source.changed().await.is_ok() {
            let arg = source.borrow().clone();
            if tx.send(map(arg).await).is_err() {
                break;
            }
        }
    });
    rx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn maps_initial_and_subsequent_values() {
        let (tx, rx) = watch::channel(1u32);
        let mut derived = derive_watch(rx, |n| async move { n * 10 }).await;
        assert_eq!(*derived.borrow(), 10);

        tx.send(5).unwrap();
        derived.changed().await.unwrap();
        assert_eq!(*derived.borrow(), 50);
    }

    #[tokio::test]
    async fn task_ends_when_source_dropped() {
        let (tx, rx) = watch::channel(0u32);
        let mut derived = derive_watch(rx, |n| async move { n + 1 }).await;
        assert_eq!(*derived.borrow(), 1);
        drop(tx);
        // No more changes once the source is gone.
        assert!(derived.changed().await.is_err());
    }
}
