//! Sync coordination for mailbox clients.
//!
//! This module provides the `SyncCoordinator`, which manages the high-level
//! sync protocol between the local node and mailbox servers. It handles
//! fetching operations for subscribed topics, updating sync watermarks,
//! and coordinating "fast-push" announcements to ensure data consistency.
//!
//! The coordinator abstracts the specific sync loop logic away from the
//! main [`crate::manager::Mailboxes`] registry.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::hash::Hash;
use std::sync::Arc;

use tokio::sync::{Mutex, mpsc};

use dashchat_utils::SeqNum;

use crate::MailboxId;
use crate::manager::TrackedMailbox;
use crate::store::MailboxStore;
use crate::sync_tracker::MailboxSyncTracker;
use crate::{FetchRequest, FetchResponse, LogHeights, MailboxClient, TopicLogs};

/// Outcome of a single [`SyncCoordinator::sync_topics`] call. The sync may
/// have failed partway through, but any topics whose receivers were found to
/// be closed are always reported so the caller can unsubscribe them.
#[derive(Debug)]
pub struct SyncTopicOutcome<Topic> {
    pub result: anyhow::Result<()>,
    pub closed_topics: HashSet<Topic>,
}

type Ack<Item> = (
    <Item as crate::MailboxItem>::Author,
    <Item as crate::MailboxItem>::LogId,
    SeqNum,
);

type TopicSenders<Item> = Mutex<HashMap<<Item as crate::MailboxItem>::Topic, mpsc::Sender<Item>>>;

#[derive(Clone)]
pub struct SyncCoordinator<Item, Store>
where
    Item: crate::MailboxItem,
    Store: MailboxStore<Item>,
{
    store: Store,
    sync_tracker: Arc<MailboxSyncTracker<Item::Author, Item::LogId>>,
}

impl<Item, Store> SyncCoordinator<Item, Store>
where
    Item: crate::MailboxItem,
    Store: MailboxStore<Item>,
{
    pub fn new(
        store: Store,
        sync_tracker: Arc<MailboxSyncTracker<Item::Author, Item::LogId>>,
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
        topic_senders: &TopicSenders<Item>,
    ) -> SyncTopicOutcome<Item::Topic>
    where
        Item::Topic: std::fmt::Display + Eq + Hash + Clone,
    {
        let (sent_logs, sent_heights) = match self.local_logs(topics).await {
            Ok(local) => local,
            Err(err) => {
                return SyncTopicOutcome {
                    result: Err(err),
                    closed_topics: HashSet::new(),
                };
            }
        };

        let request = FetchRequest {
            topics: sent_logs.keys().cloned().collect(),
            heights: sent_heights.clone(),
        };
        let FetchResponse {
            logs,
            items,
            missing,
        } = match mailbox.fetch(request).await {
            Ok(response) => response,
            Err(err) => {
                return SyncTopicOutcome {
                    result: Err(err),
                    closed_topics: HashSet::new(),
                };
            }
        };

        if items.is_empty() && missing.is_empty() {
            tracing::trace!("Syncing with mailbox: nothing to do");
        } else {
            tracing::info!(
                items = items.len(),
                missing = missing.values().map(|logs| logs.len()).sum::<usize>(),
                "fetched operations"
            );
        }

        // Sync watermark inference for logs we sent heights for:
        // if the server returned no `missing` entries for a log, it has the log
        // contiguously up to at least the height we sent.
        let mut acks: Vec<Ack<Item>> = vec![];
        for (author, heights) in &sent_heights {
            for (log_id, height) in heights {
                let reported_missing = missing
                    .get(author)
                    .is_some_and(|logs| logs.contains_key(log_id));
                if !reported_missing {
                    acks.push((*author, *log_id, *height));
                }
            }
        }

        // Each received item is one the mailbox already has.
        for item in &items {
            acks.push((item.author(), item.log_id(), item.seq_num()));
        }

        let closed_topics = deliver(items, &logs, topic_senders).await;

        let mut ops_to_publish: Vec<Item> = vec![];
        for (author, logs) in missing {
            for (log_id, seqs) in logs {
                match self.collect_missing(&author, &log_id, &seqs).await {
                    Ok(ops) => ops_to_publish.extend(ops),
                    Err(err) => {
                        return SyncTopicOutcome {
                            result: Err(err),
                            closed_topics,
                        };
                    }
                }
            }
        }

        let publish_acks: Vec<Ack<Item>> = ops_to_publish
            .iter()
            .map(|op| (op.author(), op.log_id(), op.seq_num()))
            .collect();

        if let Err(err) = mailbox.publish(sent_logs, ops_to_publish).await {
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
        log_id: Item::LogId,
    ) -> anyhow::Result<()> {
        let synced = self.sync_tracker.get_synced(id, &author, &log_id).await?;
        let start = synced.map_or(0, |n| n + 1);
        let ops = self
            .store
            .get_log(&author, &log_id, start)
            .await?
            .unwrap_or_default();
        let Some(top) = ops.iter().map(|op| op.seq_num()).max() else {
            return Ok(());
        };
        let logs: TopicLogs<Item> =
            BTreeMap::from([(topic, BTreeMap::from([(author, BTreeSet::from([log_id]))]))]);
        let response = tracked.client().await.publish(logs, ops).await?;

        let watermark = response.watermark(&author, &log_id);
        if let Some(wm) = watermark {
            self.sync_tracker
                .record_synced(id, &[(author, log_id, wm)])
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

    /// The logs the local store holds under each topic, and the heights of
    /// those logs. A log shared by several topics appears once in the heights.
    async fn local_logs(
        &self,
        topics: impl Iterator<Item = Item::Topic>,
    ) -> anyhow::Result<(TopicLogs<Item>, LogHeights<Item>)> {
        let mut logs: TopicLogs<Item> = BTreeMap::new();
        let mut heights: LogHeights<Item> = BTreeMap::new();
        for topic in topics {
            let topic_logs = logs.entry(topic).or_default();
            for (author, log_id, height) in self.store.get_log_heights(&topic).await? {
                topic_logs.entry(author).or_default().insert(log_id);
                heights.entry(author).or_default().insert(log_id, height);
            }
        }
        Ok((logs, heights))
    }

    /// The operations of one log at the given sequence numbers, which the
    /// mailbox reported missing.
    async fn collect_missing(
        &self,
        author: &Item::Author,
        log_id: &Item::LogId,
        seqs: &[SeqNum],
    ) -> anyhow::Result<Vec<Item>> {
        let Some(lowest) = seqs.iter().min() else {
            return Ok(vec![]);
        };
        let log = match self.store.get_log(author, log_id, *lowest).await {
            Ok(Some(log)) => log,
            Ok(None) => {
                tracing::error!(?author, ?log_id, ?lowest, "no log found");
                return Ok(vec![]);
            }
            Err(err) => {
                return Err(anyhow::anyhow!(
                    "failed to get log {log_id:?} of {author:?}: {err}"
                ));
            }
        };
        Ok(seqs
            .iter()
            .filter_map(|seq| log.get((seq - lowest) as usize).cloned())
            .collect())
    }
}

/// Send each fetched item to the receiver of every requested topic its log
/// belongs to. Returns the topics whose receivers turned out to be closed.
async fn deliver<Item>(
    items: Vec<Item>,
    logs: &TopicLogs<Item>,
    topic_senders: &TopicSenders<Item>,
) -> HashSet<Item::Topic>
where
    Item: crate::MailboxItem,
    Item::Topic: std::fmt::Display + Eq + Hash + Clone,
{
    let mut closed: HashSet<Item::Topic> = HashSet::new();
    if items.is_empty() {
        return closed;
    }

    let senders = topic_senders.lock().await.clone();
    let mut log_topics: HashMap<(Item::Author, Item::LogId), Vec<Item::Topic>> = HashMap::new();
    for (topic, authors) in logs {
        for (author, log_ids) in authors {
            for log_id in log_ids {
                log_topics
                    .entry((*author, *log_id))
                    .or_default()
                    .push(*topic);
            }
        }
    }

    for item in items {
        let Some(topics) = log_topics.get(&(item.author(), item.log_id())) else {
            tracing::warn!(hash = ?item.hash(), "fetched item belongs to no requested topic");
            continue;
        };
        for topic in topics {
            if closed.contains(topic) {
                continue;
            }
            let Some(sender) = senders.get(topic) else {
                tracing::warn!(topic = %topic, "no sender for topic");
                continue;
            };
            if sender.send(item.clone()).await.is_err() {
                tracing::error!(topic = %topic, "mailbox receiver closed, unsubscribing topic");
                closed.insert(*topic);
            }
        }
    }
    closed
}
