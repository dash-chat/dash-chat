//! Sync coordination for mailbox clients.
//!
//! This module provides the `SyncCoordinator`, which manages the high-level
//! sync protocol between the local node and mailbox servers. It handles
//! fetching operations for subscribed topics, updating sync watermarks,
//! and coordinating "fast-push" announcements to ensure data consistency.
//!
//! The coordinator abstracts the specific sync loop logic away from the
//! main [`crate::manager::Mailboxes`] registry.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::hash::Hash;
use std::sync::Arc;

use crate::MailboxId;
use crate::manager::TrackedMailbox;
use crate::store::MailboxStore;
use crate::sync_tracker::MailboxSyncTracker;
use crate::{FetchRequest, FetchResponse, FetchTopicResponse, MailboxClient};

/// Result of [`SyncCoordinator::sync_topics`].
#[derive(Debug)]
pub struct SyncTopicResult<Topic> {
    /// Topics whose receivers were closed during delivery.
    pub closed_topics: HashSet<Topic>,
}

impl<Topic> Default for SyncTopicResult<Topic> {
    fn default() -> Self {
        Self {
            closed_topics: HashSet::new(),
        }
    }
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
        topic_senders: &HashMap<Item::Topic, tokio::sync::mpsc::Sender<Item>>,
    ) -> anyhow::Result<SyncTopicResult<Item::Topic>>
    where
        Item::Topic: std::fmt::Display + Eq + Hash,
    {
        let mut request = BTreeMap::new();
        let mut sent_heights: BTreeMap<Item::Topic, BTreeMap<Item::Author, u64>> = BTreeMap::new();
        for topic in topics {
            let heights =
                BTreeMap::from_iter(self.store.get_log_heights(&topic).await?.into_iter());
            sent_heights.insert(topic, heights.clone());
            request.insert(topic, heights);
        }

        let FetchResponse(response) = mailbox.fetch(FetchRequest(request)).await?;

        let mut ops_to_publish: Vec<Item> = vec![];
        let mut acks: Vec<(Item::Topic, Item::Author, u64)> = vec![];
        let mut result: SyncTopicResult<Item::Topic> = SyncTopicResult::default();

        for (topic, response) in response.into_iter() {
            let FetchTopicResponse { items, missing } = response;
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
                        acks.push((topic, *author, *height));
                    }
                }
            }

            // Each received item is one the mailbox already has.
            for item in &items {
                acks.push((item.topic(), item.author(), item.seq_num()));
            }

            let Some(sender) = topic_senders.get(&topic).cloned() else {
                tracing::warn!(topic = %topic, "no sender for topic");
                continue;
            };

            for item in items {
                if sender.send(item).await.is_err() {
                    tracing::error!(topic = %topic, "mailbox receiver closed, unsubscribing topic");
                    result.closed_topics.insert(topic.clone());
                    break;
                }
            }

            for (author, seqs) in missing {
                let Some(lowest) = seqs.iter().min() else {
                    continue;
                };
                let Some(log) = self
                    .store
                    .get_log(&author, &topic, *lowest)
                    .await
                    .map_err(|err| anyhow::anyhow!("failed to get log for {topic:?}: {err}"))?
                else {
                    tracing::error!(author = ?author, topic = %topic, lowest = ?lowest, "no log found");
                    continue;
                };

                for seq in &seqs {
                    let index = seq - lowest;
                    if let Some(item) = log.get(index as usize) {
                        ops_to_publish.push(item.clone());
                    }
                }
            }
        }

        let publish_acks: Vec<(Item::Topic, Item::Author, u64)> = ops_to_publish
            .iter()
            .map(|op| (op.topic(), op.author(), op.seq_num()))
            .collect();

        mailbox.publish(ops_to_publish).await?;

        acks.extend(publish_acks);
        if let Err(err) = self.sync_tracker.record_synced(&mailbox.id(), &acks).await {
            tracing::error!(?err, mailbox = %&mailbox.id(), "failed to record sync watermarks");
        }

        Ok(result)
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
}
