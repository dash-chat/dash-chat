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
        let (reply_tx, reply_rx) = oneshot::channel();

        // Construct a node actor command.
        let payload: Payload = payload.into();

        debug!(topic = ?topic.aliased(), payload = ?payload.aliased(), "publish operation");

        let command = Command::Publish {
            topic: topic.into(),
            payload,
            reply_tx,
        };

        // Send the command to the node actor.
        if let Err(err) = self.actor_tx.send(command).await {
            tracing::warn!("failed to publish command to node actor: {}", err);
            return Err(Error::AuthorOperation(err.to_string()).into());
        }

        // Await the response, this just means that the command has been handled, it does not mean
        // the operation has been published or processed yet.
        let process_fut = warn_if_slow("awaiting reply_rx", reply_rx).await??;

        // Now we await the operation being published and processed on the system layer.
        let event = warn_if_slow("awaiting process_fut", process_fut).await?;

        // Immediately attempt to sync, skipping mailboxes that have stopped
        // retrying so a dead one can't stall the reachable ones on every send.
        self.mailboxes.attempt_immediate_sync().await;
        // Re-announce any still-unfetched blobs now that we've published.
        self.notify_unfetched_blob_followup();

        Ok(event.header().to_owned())
    }
}

async fn warn_if_slow<F: std::future::Future>(what: &str, fut: F) -> F::Output {
    tokio::pin!(fut);
    match tokio::time::timeout(std::time::Duration::from_secs(30), &mut fut).await {
        Ok(out) => out,
        Err(_) => {
            tracing::warn!("{what} is taking longer than 30s");
            fut.await
        }
    }
}
