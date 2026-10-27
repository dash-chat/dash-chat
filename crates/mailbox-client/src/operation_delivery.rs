//! Per-mailbox delivery of operations.
//!
//! This module provides `OperationDelivery`: the high-level protocol for
//! delivering operations to one mailbox server. It handles fetching
//! operations for subscribed topics and publishing the operations the
//! mailbox is missing, reporting sync watermarks for the caller to record,
//! and computes fast-push deltas (the ops above a mailbox's confirmed
//! watermark); publishing those stays with the caller.
//!
//! The delivery engine abstracts the specific sync loop logic away from the
//! `mailboxes-manager` crate's `Mailboxes` registry.

use std::collections::{BTreeMap, HashSet};
use std::hash::Hash;
use std::sync::Arc;

use dashchat_utils::SeqNum;

use crate::store::MailboxStore;
use crate::topic_registry::TopicRegistry;
use crate::{FetchRequest, FetchResponse, FetchTopicResponse, MailboxClient};

/// Outcome of a single [`OperationDelivery::sync_topics`] call. The sync may
/// have failed partway through, but any topics whose subscribers were found
/// to be closed are always reported so the caller can unsubscribe them.
#[derive(Debug)]
pub struct SyncTopicOutcome<Topic, Author> {
    /// Watermarks for the caller to record for the mailbox. A failed sync
    /// carries no acks: it records nothing.
    pub result: Result<Vec<(Topic, Author, SeqNum)>, DeliveryError>,
    pub closed_topics: HashSet<Topic>,
}

/// How delivery to the mailbox failed. The `Mailbox`/`Local` split is what
/// the caller's connection-health judgement needs: a `Mailbox` failure is
/// evidence about the mailbox's reachability; a `Local` failure (the op
/// store) says nothing about the mailbox at all.
#[derive(Debug, thiserror::Error)]
pub enum DeliveryError {
    /// The mailbox rejected or could not complete the fetch/publish exchange.
    #[error("mailbox exchange failed: {0}")]
    Mailbox(anyhow::Error),
    /// Reading the local operation store failed.
    #[error("local store failed: {0}")]
    Local(anyhow::Error),
}

impl DeliveryError {
    /// Whether this failure says anything about the mailbox's reachability.
    pub fn is_mailbox_fault(&self) -> bool {
        matches!(self, DeliveryError::Mailbox(_))
    }
}

struct SyncTopicResult<Item: crate::MailboxItem> {
    ops_to_publish: Vec<Item>,
    acks: Vec<(Item::Topic, Item::Author, SeqNum)>,
    closed: bool,
}

#[derive(Clone)]
pub struct OperationDelivery<Item, Store>
where
    Item: crate::MailboxItem,
{
    client: Arc<dyn MailboxClient<Item>>,
    store: Store,
}

impl<Item, Store> OperationDelivery<Item, Store>
where
    Item: crate::MailboxItem,
    Store: MailboxStore<Item>,
{
    pub fn new(client: Arc<dyn MailboxClient<Item>>, store: Store) -> Self {
        Self { client, store }
    }

    /// Immediately sync the given topics with the given mailbox:
    /// - Ensure all items held by the mailbox are fetched
    /// - Publish any items that the mailbox is missing to the mailbox
    ///
    /// Inbound items go straight to the topics' subscribers, snapshotted from
    /// `topic_registry` when the sync starts.
    pub async fn sync_topics(
        &self,
        topics: impl Iterator<Item = Item::Topic>,
        topic_registry: &TopicRegistry<Item>,
    ) -> SyncTopicOutcome<Item::Topic, Item::Author>
    where
        Item::Topic: std::fmt::Display + Eq + Hash + Clone,
    {
        let (response, sent_heights) = match self.fetch_missing_operations(topics).await {
            Ok(result) => result,
            Err(err) => {
                return SyncTopicOutcome {
                    result: Err(err),
                    closed_topics: HashSet::new(),
                };
            }
        };

        let mut ops_to_publish: Vec<Item> = vec![];
        let mut acks: Vec<(Item::Topic, Item::Author, SeqNum)> = vec![];
        let mut closed_topics: HashSet<Item::Topic> = HashSet::new();

        for (topic, response) in response.into_iter() {
            let sender = topic_registry.get_sender(&topic).await;
            let outcome = match self
                .sync_topic(topic.clone(), response, &sent_heights, sender)
                .await
            {
                Ok(res) => res,
                Err(err) => {
                    return SyncTopicOutcome {
                        result: Err(err),
                        closed_topics,
                    };
                }
            };

            ops_to_publish.extend(outcome.ops_to_publish);
            acks.extend(outcome.acks);
            if outcome.closed {
                closed_topics.insert(topic);
            }
        }

        let publish_acks: Vec<(Item::Topic, Item::Author, SeqNum)> = ops_to_publish
            .iter()
            .map(|op| (op.topic(), op.author(), op.seq_num()))
            .collect();

        if !ops_to_publish.is_empty() {
            if let Err(err) = self.client.publish(ops_to_publish).await {
                let err = DeliveryError::Mailbox(err);
                return SyncTopicOutcome {
                    result: Err(err),
                    closed_topics,
                };
            }

            acks.extend(publish_acks);
        }

        SyncTopicOutcome {
            result: Ok(acks),
            closed_topics,
        }
    }

    /// Collect the ops a fast push to a mailbox would send for (`topic`,
    /// `author`): everything in the local log above `synced`, the watermark
    /// the mailbox last confirmed. Publishing and recording stay with the
    /// caller.
    pub async fn fast_push_ops(
        &self,
        synced: Option<SeqNum>,
        topic: Item::Topic,
        author: Item::Author,
    ) -> Result<Vec<Item>, DeliveryError> {
        let start = synced.map_or(0, |n| n + 1);
        Ok(self
            .store
            .get_log(&author, &topic, start)
            .await
            .map_err(DeliveryError::Local)?
            .unwrap_or_default())
    }

    // PRIVATE

    /// Build a fetch request from local log heights and query the mailbox for
    /// operations it holds that we lack (and which operations we hold that it
    /// lacks). Returns the per-topic mailbox response alongside the heights we
    /// sent, which is needed later for watermark inference.
    async fn fetch_missing_operations(
        &self,
        topics: impl Iterator<Item = Item::Topic>,
    ) -> Result<
        (
            BTreeMap<Item::Topic, FetchTopicResponse<Item>>,
            BTreeMap<Item::Topic, BTreeMap<Item::Author, SeqNum>>,
        ),
        DeliveryError,
    >
    where
        Item::Topic: std::fmt::Display,
    {
        let mut request = BTreeMap::new();
        let mut sent_heights: BTreeMap<Item::Topic, BTreeMap<Item::Author, SeqNum>> =
            BTreeMap::new();
        for topic in topics {
            let heights = BTreeMap::from_iter(
                self.store
                    .get_log_heights(&topic)
                    .await
                    .map_err(DeliveryError::Local)?
                    .into_iter(),
            );
            sent_heights.insert(topic, heights.clone());
            request.insert(topic, heights);
        }

        let FetchResponse(response) = self
            .client
            .fetch(FetchRequest(request))
            .await
            .map_err(DeliveryError::Mailbox)?;
        Ok((response, sent_heights))
    }

    /// Sync a single topic with the mailbox after the fetch response has been
    /// received. Infers ack watermarks, sends received items to the local
    /// receiver, and collects missing operations from the local store for
    /// publishing.

    async fn sync_topic(
        &self,
        topic: Item::Topic,
        response: FetchTopicResponse<Item>,
        sent_heights: &BTreeMap<Item::Topic, BTreeMap<Item::Author, SeqNum>>,
        sender: Option<tokio::sync::mpsc::Sender<Item>>,
    ) -> Result<SyncTopicResult<Item>, DeliveryError>
    where
        Item::Topic: std::fmt::Display + Eq + Hash + Clone,
    {
        let FetchTopicResponse { items, missing } = response;
        let mut ops_to_publish = vec![];
        let mut acks = vec![];
        let mut closed = false;

        if items.is_empty() && missing.is_empty() {
            tracing::trace!(topic = %topic, "Syncing with mailbox: nothing to do");
        } else {
            tracing::info!(
                topic = %topic,
                items = items.len(),
                missing = missing.len(),
                "fetched operations"
            );
        }

        // Sync watermark inference for authors we sent heights for:
        // if the server returned no `missing` entries for an author, it has the log
        // contiguously up to at least the height we sent.
        if let Some(heights) = sent_heights.get(&topic) {
            for (author, height) in heights {
                if !missing.contains_key(author) {
                    acks.push((topic.clone(), *author, *height));
                }
            }
        }

        // Each received item is one the mailbox already has.
        for item in &items {
            acks.push((item.topic(), item.author(), item.seq_num()));
        }

        if let Some(sender) = sender {
            for item in &items {
                if sender.send(item.clone()).await.is_err() {
                    tracing::error!(topic = %topic, "topic receiver closed");
                    closed = true;
                    break;
                }
            }
        }

        for (author, seqs) in missing {
            let Some(lowest) = seqs.iter().min() else {
                continue;
            };
            let log = match self.store.get_log(&author, &topic, *lowest).await {
                Ok(Some(log)) => log,
                Ok(None) => {
                    tracing::error!(author = ?author, topic = %topic, lowest = ?lowest, "no log found");
                    continue;
                }
                Err(err) => {
                    return Err(DeliveryError::Local(
                        err.context(format!("failed to get log for {topic}")),
                    ));
                }
            };

            for seq in &seqs {
                let index = seq - lowest;
                if let Some(item) = log.get(index as usize) {
                    ops_to_publish.push(item.clone());
                }
            }
        }

        Ok(SyncTopicResult {
            ops_to_publish,
            acks,
            closed,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{MemStore, Msg};
    use crate::{MailboxId, PublishResponse, topic_registry::TopicRegistry};
    use std::collections::HashMap;

    fn new_delivery(
        client: Arc<dyn MailboxClient<Msg>>,
        store: MemStore,
    ) -> OperationDelivery<Msg, MemStore> {
        OperationDelivery::new(client, store)
    }

    struct NoopClient;

    #[async_trait::async_trait]
    impl MailboxClient<Msg> for NoopClient {
        fn id(&self) -> MailboxId {
            "noop".into()
        }

        async fn publish(&self, _ops: Vec<Msg>) -> Result<PublishResponse<Msg>, anyhow::Error> {
            Ok(PublishResponse::default())
        }

        async fn fetch(
            &self,
            _request: FetchRequest<Msg>,
        ) -> Result<FetchResponse<Msg>, anyhow::Error> {
            Ok(FetchResponse(Default::default()))
        }
    }

    #[tokio::test]
    async fn fast_push_ops_with_nothing_above_the_watermark_returns_empty() {
        // Watermark at the log's height: nothing newer to push.
        let store = MemStore::default();
        store.set_height(0, 'a', 4);
        let delivery = new_delivery(Arc::new(NoopClient), store);
        let ops = delivery.fast_push_ops(Some(4), 0, 'a').await.unwrap();
        assert!(ops.is_empty());

        // Log absent entirely.
        let delivery = new_delivery(Arc::new(NoopClient), MemStore::default());
        let ops = delivery.fast_push_ops(None, 0, 'a').await.unwrap();
        assert!(ops.is_empty());
    }

    #[tokio::test]
    async fn fast_push_ops_returns_the_delta_above_the_watermark() {
        let store = MemStore::default();
        store.set_height(0, 'a', 4);
        let delivery = new_delivery(Arc::new(NoopClient), store);

        let ops = delivery.fast_push_ops(Some(1), 0, 'a').await.unwrap();
        assert_eq!(
            ops,
            (2..=4)
                .map(|seq| Msg {
                    topic: 0,
                    author: 'a',
                    seq
                })
                .collect::<Vec<_>>()
        );

        // No watermark recorded yet: the whole log from zero.
        let ops = delivery.fast_push_ops(None, 0, 'a').await.unwrap();
        assert_eq!(ops.len(), 5);
        assert_eq!(ops[0].seq, 0);
    }

    struct StubClient {
        fetch_response: FetchResponse<Msg>,
        published: Arc<std::sync::Mutex<Vec<Vec<Msg>>>>,
    }

    #[async_trait::async_trait]
    impl MailboxClient<Msg> for StubClient {
        fn id(&self) -> MailboxId {
            "stub".into()
        }

        async fn publish(&self, ops: Vec<Msg>) -> Result<PublishResponse<Msg>, anyhow::Error> {
            self.published.lock().unwrap().push(ops);
            Ok(PublishResponse::default())
        }

        async fn fetch(
            &self,
            _request: FetchRequest<Msg>,
        ) -> Result<FetchResponse<Msg>, anyhow::Error> {
            Ok(self.fetch_response.clone())
        }
    }

    struct FailingFetchClient;

    #[async_trait::async_trait]
    impl MailboxClient<Msg> for FailingFetchClient {
        fn id(&self) -> MailboxId {
            "failing".into()
        }

        async fn publish(&self, _ops: Vec<Msg>) -> Result<PublishResponse<Msg>, anyhow::Error> {
            Ok(PublishResponse::default())
        }

        async fn fetch(
            &self,
            _request: FetchRequest<Msg>,
        ) -> Result<FetchResponse<Msg>, anyhow::Error> {
            anyhow::bail!("fetch failed")
        }
    }

    #[tokio::test]
    async fn sync_topics_publishes_missing_delivers_items_and_reports_acks() {
        let store = MemStore::default();
        store.set_height(0, 'a', 4);

        // The mailbox holds one op from 'b' we lack, and lacks our seqs 3-4.
        let fetch_response = FetchResponse(BTreeMap::from([(
            0u8,
            FetchTopicResponse {
                items: vec![Msg {
                    topic: 0,
                    author: 'b',
                    seq: 0,
                }],
                missing: HashMap::from([('a', vec![3, 4])]),
            },
        )]));
        let published = Arc::new(std::sync::Mutex::new(Vec::new()));
        let client: Arc<dyn MailboxClient<Msg>> = Arc::new(StubClient {
            fetch_response,
            published: published.clone(),
        });

        let topic_registry = TopicRegistry::new();
        let mut rx = topic_registry.subscribe(0).await.unwrap();

        let delivery = new_delivery(client, store);
        let outcome = delivery
            .sync_topics([0u8].into_iter(), &topic_registry)
            .await;

        // The mailbox's missing ops were published from the local log.
        assert_eq!(
            published.lock().unwrap().as_slice(),
            &[vec![
                Msg {
                    topic: 0,
                    author: 'a',
                    seq: 3
                },
                Msg {
                    topic: 0,
                    author: 'a',
                    seq: 4
                },
            ]]
        );

        // The inbound item was delivered to the topic's subscriber.
        assert_eq!(
            rx.try_recv().unwrap(),
            Msg {
                topic: 0,
                author: 'b',
                seq: 0
            }
        );

        // Received items and published ops are acked; no height inference for
        // 'a', since the mailbox reported missing seqs for them.
        assert_eq!(
            outcome.result.unwrap(),
            vec![(0, 'b', 0), (0, 'a', 3), (0, 'a', 4)]
        );
    }

    #[tokio::test]
    async fn sync_topics_reports_fetch_failure() {
        let client: Arc<dyn MailboxClient<Msg>> = Arc::new(FailingFetchClient);
        let delivery = new_delivery(client, MemStore::default());
        let topic_registry = TopicRegistry::new();

        let outcome = delivery
            .sync_topics([0u8].into_iter(), &topic_registry)
            .await;

        assert!(outcome.result.is_err());
    }

    /// A store whose reads fail, to pin the Local arm of `DeliveryError`.
    #[derive(Clone)]
    struct FailingStore;

    #[async_trait::async_trait]
    impl crate::store::MailboxStore<Msg> for FailingStore {
        async fn get_log(
            &self,
            _author: &char,
            _topic: &u8,
            _from: SeqNum,
        ) -> Result<Option<Vec<Msg>>, anyhow::Error> {
            anyhow::bail!("store read failed")
        }

        async fn get_log_heights(&self, _topic: &u8) -> Result<Vec<(char, SeqNum)>, anyhow::Error> {
            anyhow::bail!("store read failed")
        }
    }

    #[tokio::test]
    async fn delivery_error_distinguishes_mailbox_faults_from_local_ones() {
        // A fetch failure is evidence about the mailbox.
        let client: Arc<dyn MailboxClient<Msg>> = Arc::new(FailingFetchClient);
        let delivery = new_delivery(client, MemStore::default());
        let topic_registry = TopicRegistry::new();
        let outcome = delivery
            .sync_topics([0u8].into_iter(), &topic_registry)
            .await;
        let err = outcome.result.unwrap_err();
        assert!(
            err.is_mailbox_fault(),
            "fetch failure should be a mailbox fault"
        );

        // A store failure is not: the mailbox may be perfectly healthy.
        let delivery = OperationDelivery::new(Arc::new(NoopClient), FailingStore);
        let err = delivery.fast_push_ops(None, 0, 'a').await.unwrap_err();
        assert!(
            !err.is_mailbox_fault(),
            "store failure should not be blamed on the mailbox"
        );
    }
}
