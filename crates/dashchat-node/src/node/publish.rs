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
            // Boxed: this future is inlined into every caller of `publish`, and p2panda's
            // space futures make it large enough to overflow their stacks.
            Box::pin(self.streams.publish(topic.into(), payload)),
        )
        .await?;

        // Now we await the operation being published and processed on the system layer.
        let header = warn_if_slow("awaiting process_fut", process_fut).await?;

        // Re-announce any still-unfetched blobs now that we've published.
        self.notify_blobs_changed();

        Ok(header)
    }
}

impl Node {
    /// The log an author's application operations on `topic` live in, for a
    /// caller that does not know what kind of topic it holds.
    // TODO: consider newtypes/marker types for topics / logs differentiating
    // spaces from streams so that callers must "know" what kind of topic it holds.
    pub async fn app_log_id(&self, topic: TopicId) -> LogId {
        if self.streams.is_space(topic).await {
            crate::topic::application_log_id(topic)
        } else {
            LogId::from_topic(topic)
        }
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
