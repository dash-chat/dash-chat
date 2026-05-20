use std::pin::Pin;

use futures::Stream;
use p2panda::operation::Header;
use serde::{Deserialize, Serialize};

use crate::topic::AutoRegisteredTopic;

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
        self.local_store.register_topic_as_subscribed(topic).await?;
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
        todo!();
    }

    async fn initialize_topic_stream(
        &self,
        topic: TopicId,
    ) -> anyhow::Result<Option<Pin<Box<dyn Stream<Item = Operation> + Send + 'static>>>> {
        let Some(mailbox_rx) = self.mailboxes.subscribe(topic.into()).await? else {
            tracing::warn!("topic already iniitalized, skipping");
            return Ok(None);
        };

        if let Some(tx) = &self.topic_subscribed_tx {
            let _ = tx.send(topic).await;
        }

        // @TODO: import the external mailbox stream to the node.
        todo!();
    }
}
