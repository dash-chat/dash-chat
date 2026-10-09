use tracing::debug;

use crate::topic::TopicKind;

use super::*;

impl Node {
    #[tracing::instrument(skip_all, fields(me=?self.device_id().aliased()))]
    pub(super) async fn publish<K: TopicKind>(
        &self,
        topic: Topic<K>,
        payload: impl Into<Payload>,
        _alias: Option<&str>,
    ) -> Result<Header, anyhow::Error> {
        let payload: Payload = payload.into();

        debug!(topic = ?topic.aliased(), payload = ?payload.aliased(), "publish operation");

        // This only means the operation has been handed to the pipeline, not that it has been
        // published or processed yet.
        let process_fut = warn_if_slow(
            "awaiting publish",
            self.streams.publish(topic.into(), payload),
        )
        .await?;

        // Now we await the operation being published and processed on the system layer.
        let event = warn_if_slow("awaiting process_fut", process_fut).await?;

        // Re-announce any still-unfetched blobs now that we've published.
        self.notify_unfetched_blob_followup();

        Ok(event.header().to_owned())
    }
}

/// Await `fut`, warning every 30s it is still pending.
pub(super) async fn warn_if_slow<F: std::future::Future>(what: &str, fut: F) -> F::Output {
    tokio::pin!(fut);
    let started = std::time::Instant::now();
    loop {
        match tokio::time::timeout(std::time::Duration::from_secs(30), &mut fut).await {
            Ok(out) => return out,
            Err(_) => tracing::warn!(
                "{what} has been pending for {}s",
                started.elapsed().as_secs()
            ),
        }
    }
}
