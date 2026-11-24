use crate::MailboxItem;
use std::collections::{BTreeSet, HashMap, HashSet};
use tokio::sync::{Mutex, mpsc};
use tracing::info;

pub struct TopicRegistry<Item: MailboxItem> {
    topics: Mutex<HashMap<Item::Topic, mpsc::Sender<Item>>>,
}

impl<Item: MailboxItem> TopicRegistry<Item>
where
    Item::Topic: std::fmt::Display,
{
    pub fn new() -> Self {
        Self {
            topics: Mutex::new(HashMap::new()),
        }
    }

    pub async fn subscribe(&self, topic: Item::Topic) -> Option<mpsc::Receiver<Item>> {
        let mut tt = self.topics.lock().await;
        if tt.contains_key(&topic) {
            return None;
        }
        info!(topic = %topic, "subscribing to topic");
        let (tx, rx) = mpsc::channel(100);
        tt.insert(topic, tx);
        Some(rx)
    }

    pub async fn unsubscribe(&self, topic: Item::Topic) {
        info!(topic = %topic, "unsubscribing from topic");
        self.topics.lock().await.remove(&topic);
    }

    pub async fn subscribed_topics(&self) -> BTreeSet<Item::Topic> {
        self.topics.lock().await.keys().cloned().collect()
    }

    pub async fn cleanup_closed_subscriptions(&self, closed_topics: &HashSet<Item::Topic>) {
        let mut tt = self.topics.lock().await;
        for topic in closed_topics {
            if tt.get(topic).is_some_and(|s| s.is_closed()) {
                tt.remove(topic);
            }
        }
    }

    pub async fn get_sender(&self, topic: &Item::Topic) -> Option<mpsc::Sender<Item>> {
        self.topics.lock().await.get(topic).cloned()
    }

    pub async fn is_subscribed(&self, topic: &Item::Topic) -> bool {
        self.topics.lock().await.contains_key(topic)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::Msg;

    #[tokio::test]
    async fn closed_receiver_unsubscribed() {
        let registry = TopicRegistry::<Msg>::new();

        // Subscribe and immediately drop the receiver to close the channel.
        let rx = registry.subscribe(0u8).await.expect("subscribed to topic");
        drop(rx);

        // The cleanup logic should detect the closed channel.
        registry.cleanup_closed_subscriptions(&[0u8].into()).await;

        assert!(
            !registry.subscribed_topics().await.contains(&0u8),
            "closed topic should be unsubscribed"
        );
    }
}
