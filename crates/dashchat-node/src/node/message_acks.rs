use tokio::sync::Notify;

use super::*;

impl Node {
    /// Mark a chat topic as having newly processed operations for the debounced
    /// [`ChatPayload::MessageAck`] writer to cover, and wake the writer.
    pub(super) fn mark_ack_topic_dirty(&self, chat_id: ChatId) {
        self.dirty_ack_topics
            .lock()
            .expect("dirty_ack_topics lock poisoned")
            .insert(chat_id);
        self.message_ack_trigger.notify_one();
    }

    /// Publish a [`ChatPayload::MessageAck`] for `chat_id` covering everything
    /// processed but not yet acked by this device. No-op when nothing new.
    async fn publish_ack_delta(&self, chat_id: ChatId) -> anyhow::Result<()> {
        let delta = self
            .projection
            .ack_delta(chat_id.into(), self.device_id())
            .await?;

        // In a direct chat, don't reveal that we received a requester's
        // messages before their contact request is accepted. Group membership
        // implies mutual visibility, so group chats ack every member.
        let is_group_chat = self
            .projection
            .get_group_chat_ids()
            .await?
            .contains(&chat_id);
        let accepted = if is_group_chat {
            None
        } else {
            Some(self.accepted_contact_agent_ids().await?)
        };

        let mut acks = BTreeMap::new();
        for (author, acked) in delta {
            if self.author_may_be_acked(author, accepted.as_ref()).await? {
                acks.insert(author, acked);
            }
        }

        if acks.is_empty() {
            return Ok(());
        }

        tracing::debug!(chat_id = ?TopicId::from(chat_id).aliased(), entries = acks.len(), "publishing message ack");
        self.publish(
            chat_id,
            Payload::Chat(ChatPayload::MessageAck { acks }),
            None,
        )
        .await?;
        Ok(())
    }

    /// Whether `author` may appear in one of our acks: not blocked and — when
    /// `accepted` is given (direct chats) — an accepted contact.
    async fn author_may_be_acked(
        &self,
        author: DeviceId,
        accepted: Option<&BTreeSet<AgentId>>,
    ) -> anyhow::Result<bool> {
        if self.projection.is_author_blocked(&author).await? {
            return Ok(false);
        }
        let Some(accepted) = accepted else {
            return Ok(true);
        };
        Ok(matches!(
            self.projection.lookup_contact_by_device_id(author).await?,
            Some(agent_id) if accepted.contains(&agent_id)
        ))
    }

    /// Publish ack deltas for every chat topic with recorded log heads. Run
    /// once at startup: it covers acks lost to a crash between processing an
    /// operation and the debounced publish, and is a no-op otherwise.
    async fn reconcile_all_ack_topics(&self) {
        let topics = match self.projection.ack_topic_ids().await {
            Ok(topics) => topics,
            Err(err) => {
                tracing::warn!(?err, "failed to enumerate ack topics");
                return;
            }
        };
        for topic in topics {
            let Ok(chat_id) = ChatId::from_topic_id(topic) else {
                continue;
            };
            if let Err(err) = self.publish_ack_delta(chat_id).await {
                tracing::warn!(?err, topic = ?topic.aliased(), "startup ack reconciliation failed");
            }
        }
    }
}

/// Spawn the debounced [`ChatPayload::MessageAck`] writer: one startup
/// reconciliation pass, then, `debounce` after each wake from
/// [`Node::mark_ack_topic_dirty`], an ack delta for every dirty topic.
/// Runs apart from the application processor task because publishing awaits it.
pub(super) fn spawn_message_ack_task(
    node: Node,
    debounce: std::time::Duration,
    trigger: Arc<Notify>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        node.reconcile_all_ack_topics().await;
        loop {
            trigger.notified().await;
            // Debounce: let a burst of incoming operations (a sync round, a
            // mailbox poll) settle so one ack covers all of it.
            tokio::time::sleep(debounce).await;
            let topics: Vec<ChatId> = {
                let mut dirty = node
                    .dirty_ack_topics
                    .lock()
                    .expect("dirty_ack_topics lock poisoned");
                dirty.drain().collect()
            };
            for chat_id in topics {
                if let Err(err) = node.publish_ack_delta(chat_id).await {
                    tracing::warn!(?err, "failed to publish message ack");
                }
            }
        }
    })
}
