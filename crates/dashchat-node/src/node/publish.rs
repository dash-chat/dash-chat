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
            tracing::warn!("failed to send shutdown signal to node actor: {}", err);
            return Err(Error::AuthorOperation(err.to_string()).into());
        }

        // Await the response, this just means that the command has been handled, it does not mean
        // the operation has been published or processed yet.
        let process_fut = reply_rx.await??;

        // Now we await the operation being published and processed on the system layer.
        let event = process_fut.await?;

        // Trigger sync with all mailboxes.
        self.mailboxes.trigger_sync();

        Ok(event.header().to_owned())
    }
}
