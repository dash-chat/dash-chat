use std::pin::Pin;

use futures::Stream;
use futures::StreamExt;
use futures::stream::SelectAll;
use mailbox_client::MailboxItem;
use p2panda_core::Operation;
use serde::{Deserialize, Serialize};
use tokio::task;
use tokio_stream::wrappers::ReceiverStream;
use tracing::Instrument;

use crate::{payload::InboxPayload, topic::AutoRegisteredTopic};

use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Notification {
    pub header: Header,
    pub payload: Payload,
}

impl Node {
    /// Register a topic as subscribed in the database, and initialize it.
    /// When the node restarts, the topic will be reinitialized.
    ///
    /// Note that some topics are excluded from automatic registration, such as inbox topics.
    /// They have to be registered separately with extra context.
    pub(crate) async fn register_topic<K: AutoRegisteredTopic>(
        &self,
        topic: Topic<K>,
    ) -> anyhow::Result<()> {
        self.local_store.register_topic_as_subscribed(topic)?;
        self.initialize_topic(*topic).await?;

        Ok(())
    }

    /// Internal function to start the necessary tasks for processing network activity
    /// for a given topic.
    ///
    /// This must be called:
    /// - when creating a new group chat
    /// - when initializing the node, for each existing group chat
    pub(crate) async fn initialize_topic(&self, topic: TopicId) -> anyhow::Result<()> {
        let Some(mailbox_rx) = self.mailboxes.subscribe(topic.into()).await? else {
            tracing::warn!("topic already iniitalized, skipping");
            return Ok(());
        };
        let stream = ReceiverStream::new(mailbox_rx)
            .filter_map(async |op| {
                let hash = op.hash();
                if hash == op.header.hash() {
                    let header_bytes = op.header.to_bytes();
                    Some((op.header, op.body, header_bytes))
                } else {
                    tracing::error!(hash = ?hash.renamed(), "hash mismatch from mailbox server");
                    None
                }
            })
            .ingest(self.op_store.clone(), 128)
            .filter_map(|result| async {
                match result {
                    Ok(operation) => Some(operation),
                    Err(err) => {
                        tracing::warn!(?err, "ingest operation error");
                        None
                    }
                }
            });

        self.stream_tx
            .send(Pin::from(Box::new(stream)))
            .await
            .map_err(|_| anyhow::anyhow!("stream channel closed"))?;

        Ok(())
    }

    pub(super) fn spawn_stream_process_loop(
        &self,
        mut stream_rx: mpsc::Receiver<
            Pin<Box<dyn Stream<Item = Operation<Extensions>> + Send + 'static>>,
        >,
    ) -> CancelAndWait<()> {
        let node = self.clone();

        let token = tokio_util::sync::CancellationToken::new();
        let token2 = token.clone();

        let handle = task::spawn(
            async move {
                let node = node.clone();
                let mut streams = SelectAll::new();

                loop {
                    tokio::select! {
                        Some(stream) = stream_rx.recv() => {
                            tracing::info!("received new STREAM");
                            // Stream is already Pin<Box<...>> from the channel, push directly
                            streams.push(stream);
                        }

                        Some(op) = streams.next() => {
                            tracing::info!(op = ?op.hash.renamed(), topic = ?op.header.extensions.topic.renamed(), "processing stream item");
                            // Process the FromNetwork item here
                            if let Err(err) = node.process_stream_item(op).await {
                                tracing::error!(?err, "process stream item error");
                            }
                        }

                        _ = token.cancelled() => {
                            tracing::info!("stream processing loop cancelled");
                            break;
                        }

                        else => {
                            // Both stream_rx is closed and streams is exhausted
                            break;
                        }
                    }
                }
            }
            .instrument(tracing::info_span!("stream_process_loop")),
        );

        CancelAndWait::new(handle, token2)
    }

    async fn process_stream_item(&self, operation: Operation<Extensions>) -> anyhow::Result<()> {
        let hash = operation.hash;
        let topic = operation.header.extensions.topic;

        if let Err(err) = self.op_store.process_ordering(operation).await {
            tracing::error!(?err, "process ordering error");
        }

        let reordered = self
            .op_store
            .next_ordering()
            .await
            .map_err(|err| {
                tracing::error!(?err, "next ordering error");
            })
            .unwrap_or_default();

        // let reordered = vec![operation];

        for operation in reordered {
            match self.process_operation(operation, false, false).await {
                Ok(()) => (),
                Err(err) => {
                    tracing::error!(
                        ?topic,
                        hash = ?hash.renamed(),
                        ?err,
                        "process operation error"
                    )
                }
            }
        }
        Ok(())
    }

    pub async fn process_operation(
        &self,
        // topic: Topic<K>,
        operation: Operation<Extensions>,
        is_author: bool,
        _is_repair: bool,
    ) -> anyhow::Result<()> {
        let Operation { header, body, hash } = operation;

        let topic = header.extensions.topic;

        // XXX: this eventually needs to be more selective than just adding any old author
        // author_store.add_author(topic, header.public_key).await;
        tracing::debug!(?topic, "adding author");

        tracing::info!(topic = ?topic.renamed(), hash = ?hash.renamed(), "PROC: processing operation");

        let payload = body.map(|body| Payload::try_from_body(&body)).transpose()?;

        tracing::trace!(?payload, "RECEIVED PAYLOAD");

        // if !is_repair {
        if let Err(err) = self
            .process_payload(&header, payload.as_ref(), is_author)
            .await
        {
            tracing::error!(
                hash = ?header.hash().renamed(),
                ?payload,
                ?err,
                "process operation error"
            );
            return Err(err);
        }

        tracing::info!(hash = ?hash.renamed(), "processed operation");

        if let Some(payload) = payload.as_ref() {
            self.notify_payload(&header, payload).await?;
        }

        // XXX: don't repair this often.
        // Box::pin(self.repair_spaces_and_publish()).await?;

        self.op_store.mark_op_processed(topic, &hash);

        anyhow::Ok(())
    }

    pub async fn notify_payload(&self, header: &Header, payload: &Payload) -> anyhow::Result<()> {
        if let Some((notification_tx, payload)) = self.notification_tx.clone().zip(Some(payload)) {
            notification_tx
                .send(Notification {
                    header: header.clone(),
                    payload: payload.clone(),
                })
                .await
                .unwrap_or_else(|_| tracing::warn!("notification channel closed"));
        }
        Ok(())
    }

    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all, fields(me=?self.device_id().renamed())))]
    pub async fn process_payload(
        &self,
        // topic: Topic<K>,
        header: &Header,
        payload: Option<&Payload>,
        _is_author: bool,
    ) -> anyhow::Result<()> {
        let topic = header.extensions.topic;
        // TODO: maybe have different loops for the different kinds of topics and the different payloads in each
        match &payload {
            Some(Payload::Chat(ChatPayload::JoinGroup(_chat_id))) => {
                // TODO: maybe close down the chat tasks if we are kicked out?
            }

            Some(Payload::Inbox(invitation)) => {
                let active_topics = self.local_store.get_active_inbox_topics()?;
                if !active_topics.iter().any(|it| **it.topic == *topic) {
                    // not for me, ignore
                    return Ok(());
                }
                tracing::info!(
                    ?invitation,
                    from = ?header.public_key.renamed(),
                    "received invitation message"
                );
                match invitation {
                    InboxPayload::ContactRequest { .. } => {
                        // Nothing to do.
                    }
                }
            }

            Some(Payload::Chat(ChatPayload::Message(_) | ChatPayload::Reaction(_))) => {
                // Nothing to do.
            }

            Some(Payload::Announcements(_)) => {
                // Nothing to do.
            }

            Some(Payload::DeviceGroup(_)) => {
                // Nothing to do.
            }

            None => {
                tracing::error!(?topic, "no payload");
            }
        }
        Ok(())
    }
}
