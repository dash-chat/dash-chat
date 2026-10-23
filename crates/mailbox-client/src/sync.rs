//! Sync coordination for mailbox clients.
//!
//! This module provides the `SyncCoordinator`, which manages the high-level
//! sync protocol between the local node and mailbox servers. It handles
//! fetching operations for subscribed topics and publishing the operations
//! the mailbox is missing, reporting sync watermarks and fast-push outcomes
//! for the caller to record.
//!
//! The coordinator abstracts the specific sync loop logic away from the
//! `mailboxes-manager` crate's `Mailboxes` registry.

use std::collections::{BTreeMap, HashSet};
use std::hash::Hash;
use std::sync::Arc;

use dashchat_utils::SeqNum;

use crate::store::MailboxStore;
use crate::topic_registry::TopicRegistry;
use crate::{FetchRequest, FetchResponse, FetchTopicResponse, MailboxClient};

/// Outcome of a single [`SyncCoordinator::sync_topics`] call. The sync may
/// have failed partway through, but any topics whose receivers were found to
/// be closed are always reported so the caller can unsubscribe them.
#[derive(Debug)]
pub struct SyncTopicOutcome<Topic, Author> {
    /// Watermarks for the caller to record for the mailbox. A failed sync
    /// carries no acks: it records nothing.
    pub result: anyhow::Result<Vec<(Topic, Author, SeqNum)>>,
    pub closed_topics: HashSet<Topic>,
}

struct SyncTopicResult<Item: crate::MailboxItem> {
    ops_to_publish: Vec<Item>,
    acks: Vec<(Item::Topic, Item::Author, SeqNum)>,
    closed: bool,
}

#[derive(Clone)]
pub struct SyncCoordinator<Item, Store> {
    store: Store,
    phantom: std::marker::PhantomData<fn() -> Item>,
}

impl<Item, Store> SyncCoordinator<Item, Store>
where
    Item: crate::MailboxItem,
    Store: MailboxStore<Item>,
{
    pub fn new(store: Store) -> Self {
        Self {
            store,
            phantom: std::marker::PhantomData,
        }
    }

    /// Immediately sync the given topics with the given mailbox:
    /// - Ensure all items held by the mailbox are fetched
    /// - Publish any items that the mailbox is missing to the mailbox
    pub async fn sync_topics(
        &self,
        topics: impl Iterator<Item = Item::Topic>,
        mailbox: &Arc<dyn MailboxClient<Item>>,
        topic_registry: &TopicRegistry<Item>,
    ) -> SyncTopicOutcome<Item::Topic, Item::Author>
    where
        Item::Topic: std::fmt::Display + Eq + Hash + Clone,
    {
        let (response, sent_heights) = match self.fetch_missing_operations(topics, mailbox).await {
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
            let outcome = match self
                .sync_topic(topic.clone(), response, &sent_heights, topic_registry)
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

        if let Err(err) = mailbox.publish(ops_to_publish).await {
            return SyncTopicOutcome {
                result: Err(err),
                closed_topics,
            };
        }

        acks.extend(publish_acks);

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
    ) -> anyhow::Result<Vec<Item>> {
        let start = synced.map_or(0, |n| n + 1);
        Ok(self
            .store
            .get_log(&author, &topic, start)
            .await?
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
        mailbox: &Arc<dyn MailboxClient<Item>>,
    ) -> anyhow::Result<(
        BTreeMap<Item::Topic, FetchTopicResponse<Item>>,
        BTreeMap<Item::Topic, BTreeMap<Item::Author, SeqNum>>,
    )>
    where
        Item::Topic: std::fmt::Display,
    {
        let mut request = BTreeMap::new();
        let mut sent_heights: BTreeMap<Item::Topic, BTreeMap<Item::Author, SeqNum>> =
            BTreeMap::new();
        for topic in topics {
            let heights =
                BTreeMap::from_iter(self.store.get_log_heights(&topic).await?.into_iter());
            sent_heights.insert(topic, heights.clone());
            request.insert(topic, heights);
        }

        let FetchResponse(response) = mailbox.fetch(FetchRequest(request)).await?;
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
        topic_registry: &TopicRegistry<Item>,
    ) -> anyhow::Result<SyncTopicResult<Item>>
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

        let Some(sender) = topic_registry.get_sender(&topic).await else {
            tracing::warn!(topic = %topic, "no sender for topic");
            return Ok(SyncTopicResult {
                ops_to_publish,
                acks,
                closed,
            });
        };

        for item in items {
            if sender.send(item).await.is_err() {
                tracing::error!(topic = %topic, "mailbox receiver closed, unsubscribing topic");
                closed = true;
                break;
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
                    return Err(anyhow::anyhow!("failed to get log for {topic:?}: {err}"));
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

    fn new_coordinator(store: MemStore) -> SyncCoordinator<Msg, MemStore> {
        SyncCoordinator::new(store)
    }

    #[tokio::test]
    async fn fast_push_ops_with_nothing_above_the_watermark_returns_empty() {
        // Watermark at the log's height: nothing newer to push.
        let store = MemStore::default();
        store.set_height(0, 'a', 4);
        let coordinator = new_coordinator(store);
        let ops = coordinator.fast_push_ops(Some(4), 0, 'a').await.unwrap();
        assert!(ops.is_empty());

        // Log absent entirely.
        let coordinator = new_coordinator(MemStore::default());
        let ops = coordinator.fast_push_ops(None, 0, 'a').await.unwrap();
        assert!(ops.is_empty());
    }

    #[tokio::test]
    async fn fast_push_ops_returns_the_delta_above_the_watermark() {
        let store = MemStore::default();
        store.set_height(0, 'a', 4);
        let coordinator = new_coordinator(store);

        let ops = coordinator.fast_push_ops(Some(1), 0, 'a').await.unwrap();
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
        let ops = coordinator.fast_push_ops(None, 0, 'a').await.unwrap();
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
        let coordinator = new_coordinator(store);

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

        let outcome = coordinator
            .sync_topics([0u8].into_iter(), &client, &topic_registry)
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

        // The inbound item was delivered to the topic's receiver.
        assert_eq!(
            rx.try_recv().unwrap(),
            Msg {
                topic: 0,
                author: 'b',
                seq: 0
            }
        );

        assert!(outcome.closed_topics.is_empty());
        // Received items and published ops are acked; no height inference for
        // 'a', since the mailbox reported missing seqs for them.
        assert_eq!(
            outcome.result.unwrap(),
            vec![(0, 'b', 0), (0, 'a', 3), (0, 'a', 4)]
        );
    }

    #[tokio::test]
    async fn sync_topics_reports_fetch_failure() {
        let coordinator = new_coordinator(MemStore::default());
        let client: Arc<dyn MailboxClient<Msg>> = Arc::new(FailingFetchClient);
        let topic_registry = TopicRegistry::new();
        topic_registry.subscribe(0).await.unwrap();

        let outcome = coordinator
            .sync_topics([0u8].into_iter(), &client, &topic_registry)
            .await;

        assert!(outcome.result.is_err());
        assert!(outcome.closed_topics.is_empty());
    }
}
