//! Sync coordination for mailbox clients.
//!
//! This module provides the `SyncCoordinator`, which manages the high-level
//! sync protocol between the local node and mailbox servers. It handles
//! fetching operations for subscribed topics, updating sync watermarks,
//! and coordinating "fast-push" announcements to ensure data consistency.
//!
//! The coordinator abstracts the specific sync loop logic away from the
//! main [`crate::manager::Mailboxes`] registry.

use std::collections::{BTreeMap, HashSet};
use std::hash::Hash;
use std::sync::Arc;

use dashchat_utils::SeqNum;

use crate::MailboxId;
use crate::manager::TrackedMailbox;
use crate::store::MailboxStore;
use crate::sync_tracker::MailboxSyncTracker;
use crate::topic_registry::TopicRegistry;
use crate::{FetchRequest, FetchResponse, FetchTopicResponse, MailboxClient};

/// Outcome of a single [`SyncCoordinator::sync_topics`] call. The sync may
/// have failed partway through, but any topics whose receivers were found to
/// be closed are always reported so the caller can unsubscribe them.
#[derive(Debug)]
pub struct SyncTopicOutcome<Topic> {
    pub result: anyhow::Result<()>,
    pub closed_topics: HashSet<Topic>,
}

struct SyncTopicResult<Item: crate::MailboxItem> {
    ops_to_publish: Vec<Item>,
    acks: Vec<(Item::Topic, Item::Author, SeqNum)>,
    closed: bool,
}

#[derive(Clone)]
pub struct SyncCoordinator<Item, Store>
where
    Item: crate::MailboxItem,
    Store: MailboxStore<Item>,
{
    store: Store,
    sync_tracker: Arc<MailboxSyncTracker<Item::Topic, Item::Author>>,
}

impl<Item, Store> SyncCoordinator<Item, Store>
where
    Item: crate::MailboxItem,
    Store: MailboxStore<Item>,
{
    pub fn new(
        store: Store,
        sync_tracker: Arc<MailboxSyncTracker<Item::Topic, Item::Author>>,
    ) -> Self {
        Self {
            store,
            sync_tracker,
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
    ) -> SyncTopicOutcome<Item::Topic>
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
        if let Err(err) = self.sync_tracker.record_synced(&mailbox.id(), &acks).await {
            tracing::error!(?err, mailbox = %&mailbox.id(), "failed to record sync watermarks");
        }

        SyncTopicOutcome {
            result: Ok(()),
            closed_topics,
        }
    }

    pub async fn store_fast_push(
        &self,
        id: &MailboxId,
        tracked: &TrackedMailbox<Item>,
        topic: Item::Topic,
        author: Item::Author,
    ) -> anyhow::Result<()> {
        let synced = self.sync_tracker.get_synced(id, &topic, &author).await?;
        let start = synced.map_or(0, |n| n + 1);
        let ops = self
            .store
            .get_log(&author, &topic, start)
            .await?
            .unwrap_or_default();
        let Some(top) = ops.iter().map(|op| op.seq_num()).max() else {
            return Ok(());
        };
        let response = tracked.client().await.publish(ops).await?;

        let watermark = response.watermark(&topic, &author);
        if let Some(wm) = watermark {
            self.sync_tracker
                .record_synced(id, &[(topic, author, wm)])
                .await?;
        }
        if watermark.is_none_or(|wm| wm < top) {
            tracing::warn!(
                mailbox = %id,
                pushed = top,
                echoed = ?watermark,
                "mailbox watermark behind after direct store"
            );
            anyhow::bail!("mailbox watermark {watermark:?} behind published seq {top}");
        }
        Ok(())
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
            if sender.send(item.clone()).await.is_err() {
                tracing::error!(topic = %topic, "mailbox receiver closed, unsubscribing topic");
                closed = true;
                break;
            }
            ops_to_publish.push(item);
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
