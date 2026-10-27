//! Per-mailbox delivery of operations.
//!
//! This module provides `OperationDelivery`: the high-level protocol for
//! delivering operations to one mailbox server. It handles fetching
//! operations for subscribed topics, publishing the operations the mailbox
//! is missing (in syncs and direct pushes), and reporting sync watermarks
//! and confirmed echoes for the caller to record.
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

/// Outcome of a single [`OperationDelivery::sync`] call. The sync may
/// have failed partway through, but any topics whose subscribers were found
/// to be closed are always reported so the caller can unsubscribe them.
#[derive(Debug)]
pub struct SyncOutcome<Topic, Author> {
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
    /// The publish succeeded but the mailbox's echoed watermark fell short of
    /// what was pushed: its log is not contiguous up to the pushed seq, so
    /// the watermark must not advance past the echo.
    #[error("mailbox watermark {echoed:?} behind published seq {pushed}")]
    WatermarkBehind {
        pushed: SeqNum,
        echoed: Option<SeqNum>,
    },
}

impl DeliveryError {
    /// Whether this failure says anything about the mailbox's reachability or
    /// integrity. A short watermark echo is the mailbox's doing as much as a
    /// failed request is.
    pub fn is_mailbox_fault(&self) -> bool {
        !matches!(self, DeliveryError::Local(_))
    }
}

struct SyncResult<Item: crate::MailboxItem> {
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
    Item::Topic: std::fmt::Display,
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
    pub async fn sync(
        &self,
        topics: impl Iterator<Item = Item::Topic>,
        topic_registry: &TopicRegistry<Item>,
    ) -> SyncOutcome<Item::Topic, Item::Author> {
        let (response, sent_heights) = match self.fetch_missing_operations(topics).await {
            Ok(result) => result,
            Err(err) => {
                return SyncOutcome {
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
                .sync_one_topic(topic.clone(), response, &sent_heights, sender)
                .await
            {
                Ok(res) => res,
                Err(err) => {
                    return SyncOutcome {
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
            let response = match self.client.publish(ops_to_publish).await {
                Ok(response) => response,
                Err(err) => {
                    return SyncOutcome {
                        result: Err(DeliveryError::Mailbox(err)),
                        closed_topics,
                    };
                }
            };

            // Only confirmed ops are acked: a log whose echoed watermark is
            // short (or absent) contributes nothing and fails the sync, so
            // the caller retries it instead of trusting unconfirmed sends.
            let mut short: Option<(Item::Topic, Item::Author, SeqNum, Option<SeqNum>)> = None;
            for (topic, author, seq) in publish_acks {
                let echoed = response.watermark(&topic, &author);
                if echoed.is_some_and(|wm| wm >= seq) {
                    acks.push((topic, author, seq));
                } else if short.is_none() {
                    short = Some((topic, author, seq, echoed));
                }
            }
            if let Some((topic, author, pushed, echoed)) = short {
                tracing::warn!(
                    %topic,
                    pushed,
                    echoed = ?echoed,
                    "mailbox watermark behind after sync publish"
                );
                let _ = author;
                return SyncOutcome {
                    result: Err(DeliveryError::WatermarkBehind { pushed, echoed }),
                    closed_topics,
                };
            }
        }

        SyncOutcome {
            result: Ok(acks),
            closed_topics,
        }
    }

    /// New ops exist locally for (`topic`, `author`). Push everything above
    /// `synced` — the watermark the mailbox last confirmed — and return the
    /// watermark the mailbox echoed back. `Ok(None)` means the log had
    /// nothing new. Fails with [`DeliveryError::WatermarkBehind`] when the
    /// echo falls short of the pushed top.
    pub async fn publish(
        &self,
        topic: Item::Topic,
        author: Item::Author,
        synced: Option<SeqNum>,
    ) -> Result<Option<SeqNum>, DeliveryError> {
        let ops = self.fast_push_ops(synced, topic, author).await?;
        let Some(top) = ops.iter().map(|op| op.seq_num()).max() else {
            return Ok(None);
        };
        let response = self
            .client
            .publish(ops)
            .await
            .map_err(DeliveryError::Mailbox)?;
        let echoed = response.watermark(&topic, &author);
        if echoed.is_none_or(|wm| wm < top) {
            return Err(DeliveryError::WatermarkBehind {
                pushed: top,
                echoed,
            });
        }
        Ok(echoed)
    }

    // PRIVATE

    /// Collect the ops a push to the mailbox would send for (`topic`,
    /// `author`): everything in the local log above `synced`, the watermark
    /// the mailbox last confirmed.
    async fn fast_push_ops(
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

    async fn sync_one_topic(
        &self,
        topic: Item::Topic,
        response: FetchTopicResponse<Item>,
        sent_heights: &BTreeMap<Item::Topic, BTreeMap<Item::Author, SeqNum>>,
        sender: Option<tokio::sync::mpsc::Sender<Item>>,
    ) -> Result<SyncResult<Item>, DeliveryError>
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

        Ok(SyncResult {
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
    use crate::{MailboxId, MailboxItem, PublishResponse, topic_registry::TopicRegistry};
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
            let mut response = PublishResponse::default();
            for op in &ops {
                // Echo each published seq: this stub stores contiguously.
                response
                    .0
                    .entry(op.topic())
                    .or_default()
                    .insert(op.author(), Some(op.seq_num()));
            }
            self.published.lock().unwrap().push(ops);
            Ok(response)
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
    async fn sync_publishes_missing_delivers_items_and_reports_acks() {
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
        let outcome = delivery.sync([0u8].into_iter(), &topic_registry).await;

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
    async fn sync_reports_fetch_failure() {
        let client: Arc<dyn MailboxClient<Msg>> = Arc::new(FailingFetchClient);
        let delivery = new_delivery(client, MemStore::default());
        let topic_registry = TopicRegistry::new();

        let outcome = delivery.sync([0u8].into_iter(), &topic_registry).await;

        assert!(outcome.result.is_err());
    }

    /// A client that echoes a watermark for published ops.
    struct EchoClient {
        watermark: Option<SeqNum>,
        published: Arc<std::sync::Mutex<Vec<Vec<Msg>>>>,
    }

    #[async_trait::async_trait]
    impl MailboxClient<Msg> for EchoClient {
        fn id(&self) -> MailboxId {
            "echo".into()
        }

        async fn publish(&self, ops: Vec<Msg>) -> Result<PublishResponse<Msg>, anyhow::Error> {
            self.published.lock().unwrap().push(ops);
            let mut response = PublishResponse::default();
            if let Some(wm) = self.watermark {
                response.0.entry(0u8).or_default().insert('a', Some(wm));
            }
            Ok(response)
        }

        async fn fetch(
            &self,
            _request: FetchRequest<Msg>,
        ) -> Result<FetchResponse<Msg>, anyhow::Error> {
            Ok(FetchResponse(Default::default()))
        }
    }

    fn publish_setup(
        height: SeqNum,
        watermark: Option<SeqNum>,
    ) -> (
        OperationDelivery<Msg, MemStore>,
        Arc<std::sync::Mutex<Vec<Vec<Msg>>>>,
    ) {
        let store = MemStore::default();
        store.set_height(0, 'a', height);
        let published = Arc::new(std::sync::Mutex::new(Vec::new()));
        let delivery = new_delivery(
            Arc::new(EchoClient {
                watermark,
                published: published.clone(),
            }),
            store,
        );
        (delivery, published)
    }

    #[tokio::test]
    async fn publish_pushes_the_delta_and_returns_the_echoed_watermark() {
        let (delivery, published) = publish_setup(4, Some(4));

        let result = delivery.publish(0, 'a', Some(1)).await.unwrap();

        assert_eq!(result, Some(4));
        let pushed = published.lock().unwrap();
        assert_eq!(published_seqs(&pushed), vec![vec![2, 3, 4]]);
    }

    #[tokio::test]
    async fn publish_with_a_short_echo_fails_and_advances_nothing() {
        let (delivery, published) = publish_setup(4, Some(2));

        let err = delivery.publish(0, 'a', Some(1)).await.unwrap_err();

        assert!(matches!(
            err,
            DeliveryError::WatermarkBehind {
                pushed: 4,
                echoed: Some(2)
            }
        ));
        assert!(
            err.is_mailbox_fault(),
            "a short echo is evidence about the mailbox"
        );
        assert_eq!(published.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn publish_with_nothing_new_publishes_nothing() {
        let (delivery, published) = publish_setup(4, Some(4));

        let result = delivery.publish(0, 'a', Some(4)).await.unwrap();

        assert_eq!(result, None);
        assert!(published.lock().unwrap().is_empty());
    }

    fn published_seqs(published: &[Vec<Msg>]) -> Vec<Vec<SeqNum>> {
        published
            .iter()
            .map(|batch| batch.iter().map(|m| m.seq).collect())
            .collect()
    }

    /// A client whose publish echoes a watermark short of the pushed top.
    struct ShortEchoClient;

    #[async_trait::async_trait]
    impl MailboxClient<Msg> for ShortEchoClient {
        fn id(&self) -> MailboxId {
            "short-echo".into()
        }

        async fn publish(&self, ops: Vec<Msg>) -> Result<PublishResponse<Msg>, anyhow::Error> {
            let mut response = PublishResponse::default();
            for op in &ops {
                let short = op.seq_num().saturating_sub(1);
                response
                    .0
                    .entry(op.topic())
                    .or_default()
                    .insert(op.author(), Some(short));
            }
            Ok(response)
        }

        async fn fetch(
            &self,
            request: FetchRequest<Msg>,
        ) -> Result<FetchResponse<Msg>, anyhow::Error> {
            // Report everything advertised as missing, so the sync publishes.
            let response = request
                .0
                .iter()
                .map(|(topic, authors)| {
                    let missing = authors
                        .iter()
                        .map(|(author, height)| (*author, vec![*height]))
                        .collect();
                    (
                        *topic,
                        FetchTopicResponse {
                            items: Vec::new(),
                            missing,
                        },
                    )
                })
                .collect();
            Ok(FetchResponse(response))
        }
    }

    #[tokio::test]
    async fn sync_with_a_short_publish_echo_fails_and_acks_nothing_published() {
        let store = MemStore::default();
        store.set_height(0, 'a', 2);
        let delivery = new_delivery(Arc::new(ShortEchoClient), store);
        let topic_registry = TopicRegistry::new();

        let outcome = delivery.sync([0u8].into_iter(), &topic_registry).await;

        assert!(matches!(
            outcome.result.unwrap_err(),
            DeliveryError::WatermarkBehind {
                pushed: 2,
                echoed: Some(1)
            }
        ));
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
        let outcome = delivery.sync([0u8].into_iter(), &topic_registry).await;
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
