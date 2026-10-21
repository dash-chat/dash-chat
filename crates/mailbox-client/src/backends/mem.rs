use crate::{
    FetchRequest, FetchResponse, MailboxClient, MailboxId, MailboxItem, OptionalItemTraits,
    PublishResponse, TopicLogs,
};

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    sync::Arc,
};

use dashchat_utils::SeqNum;
use tokio::sync::RwLock;

/// A client for the in-memory mailbox server.
/// This client is stateful, so all requests for a node should go through a single client
/// instance. State is shared between all cloned copies of this.
#[derive(Clone)]
pub struct MemMailboxClient<Item: MailboxItem> {
    mailbox: MemMailbox<Item>,
    subscribed_topics: Arc<RwLock<BTreeSet<Item::Topic>>>,
}

impl<Item: MailboxItem> MemMailboxClient<Item> {
    pub async fn subscribed_topics(&self) -> BTreeSet<Item::Topic> {
        self.subscribed_topics.read().await.clone()
    }
}

type Log<Item> = (<Item as MailboxItem>::Author, <Item as MailboxItem>::LogId);

/// What the in-memory mailbox holds: which logs belong to which topic, and
/// the logs themselves, stored once however many topics they belong to.
#[derive(Clone)]
pub struct MemMailboxLogs<Item: MailboxItem> {
    pub topics: TopicLogs<Item>,
    pub ops: HashMap<Log<Item>, BTreeMap<SeqNum, Item>>,
}

impl<Item: MailboxItem> Default for MemMailboxLogs<Item> {
    fn default() -> Self {
        Self {
            topics: BTreeMap::new(),
            ops: HashMap::new(),
        }
    }
}

#[derive(Clone)]
pub struct MemMailbox<Item: MailboxItem> {
    id: MailboxId,
    logs: Arc<RwLock<MemMailboxLogs<Item>>>,
}

impl<Item: MailboxItem> Default for MemMailbox<Item> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Item: MailboxItem> MemMailbox<Item> {
    pub fn new() -> Self {
        Self {
            id: nanoid::nanoid!(),
            logs: Arc::new(RwLock::new(MemMailboxLogs::default())),
        }
    }

    pub fn client(&self) -> MemMailboxClient<Item> {
        MemMailboxClient {
            mailbox: self.clone(),
            subscribed_topics: Arc::new(RwLock::new(BTreeSet::new())),
        }
    }

    /// The number of operations held per log.
    pub async fn log_heights(&self) -> BTreeMap<Item::Author, BTreeMap<Item::LogId, SeqNum>> {
        let logs = self.logs.read().await;
        let mut log_heights: BTreeMap<Item::Author, BTreeMap<Item::LogId, SeqNum>> =
            BTreeMap::new();
        for ((author, log_id), ops) in logs.ops.iter() {
            log_heights
                .entry(*author)
                .or_default()
                .insert(*log_id, ops.len() as SeqNum);
        }
        log_heights
    }
}

#[async_trait::async_trait]
impl<Item: MailboxItem> MailboxClient<Item> for MemMailboxClient<Item>
where
    Item::Topic: OptionalItemTraits,
    Item::Hash: OptionalItemTraits,
{
    fn id(&self) -> MailboxId {
        self.mailbox.id.clone()
    }

    async fn publish(
        &self,
        logs: TopicLogs<Item>,
        ops: Vec<Item>,
    ) -> Result<PublishResponse<Item>, anyhow::Error> {
        let mut store = self.mailbox.logs.write().await;
        for (topic, authors) in logs {
            let topic_logs = store.topics.entry(topic).or_default();
            for (author, log_ids) in authors {
                topic_logs.entry(author).or_default().extend(log_ids);
            }
        }

        let mut published: BTreeSet<Log<Item>> = BTreeSet::new();
        for op in ops {
            let log = (op.author(), op.log_id());
            let seq_num = op.seq_num();
            tracing::info!(log = ?log, hash = ?op.hash(), "publishing mailbox operation");
            store.ops.entry(log).or_default().insert(seq_num, op);
            published.insert(log);
        }

        let mut response = PublishResponse::default();
        for (author, log_id) in published {
            let watermark = store
                .ops
                .get(&(author, log_id))
                .and_then(contiguous_watermark);
            response
                .0
                .entry(author)
                .or_default()
                .insert(log_id, watermark);
        }
        Ok(response)
    }

    async fn fetch(&self, request: FetchRequest<Item>) -> anyhow::Result<FetchResponse<Item>> {
        let store = self.mailbox.logs.read().await;

        let mut response = FetchResponse::default();
        let mut served: BTreeSet<Log<Item>> = BTreeSet::new();
        for topic in request.topics {
            let logs = store.topics.get(&topic).cloned().unwrap_or_default();
            for (author, log_ids) in &logs {
                for log_id in log_ids {
                    served.insert((*author, *log_id));
                }
            }
            response.logs.insert(topic, logs);
        }

        let provided: BTreeSet<Log<Item>> = request
            .heights
            .iter()
            .flat_map(|(author, logs)| logs.keys().map(move |log_id| (*author, *log_id)))
            .collect();

        for log in served.union(&provided) {
            let (author, log_id) = *log;
            let mut gaps = vec![];
            let mailbox_slots = store.ops.get(log).filter(|_| served.contains(log));
            let provided_height = request
                .heights
                .get(&author)
                .and_then(|logs| logs.get(&log_id))
                .copied();

            match (mailbox_slots, provided_height) {
                (Some(mailbox_slots), Some(provided_height)) => {
                    let last_seq = mailbox_slots
                        .last_key_value()
                        .map(|(seq, _)| *seq)
                        .unwrap_or(0);

                    let max = provided_height.max(last_seq);
                    for i in 0..=max {
                        if let Some(op) = mailbox_slots.get(&i) {
                            if i > provided_height {
                                response.items.push(op.clone());
                            }
                        } else {
                            gaps.push(i);
                        }
                    }
                }
                (Some(mailbox_slots), None) => {
                    response.items.extend(mailbox_slots.values().cloned());
                }
                (None, Some(provided_height)) => {
                    let held = store.ops.get(log);
                    gaps.extend(
                        (0..=provided_height)
                            .filter(|seq| !held.is_some_and(|slots| slots.contains_key(seq))),
                    );
                }
                (None, None) => {
                    // empty mailbox, empty node store, return nothing...
                }
            }
            if !gaps.is_empty() {
                response
                    .missing
                    .entry(author)
                    .or_default()
                    .insert(log_id, gaps);
            }
        }

        tracing::info!(
            num = response.items.len(),
            hashes = ?response.items.iter().map(|op: &Item| op.hash()).collect::<Vec<_>>(),
            "fetching mailbox operations"
        );

        Ok(FetchResponse {
            logs: response.logs,
            items: response.items,
            missing: response.missing,
        })
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use crate::testing::Msg;

    use super::*;

    fn msg(topic: u8, author: char, seq: SeqNum) -> Msg {
        Msg { topic, author, seq }
    }

    /// `Msg`s live in the log named after their topic.
    fn logs(msgs: &[Msg]) -> TopicLogs<Msg> {
        let mut logs: TopicLogs<Msg> = BTreeMap::new();
        for m in msgs {
            logs.entry(m.topic)
                .or_default()
                .entry(m.author)
                .or_default()
                .insert(m.topic);
        }
        logs
    }

    async fn publish(client: &MemMailboxClient<Msg>, msgs: Vec<Msg>) -> PublishResponse<Msg> {
        client.publish(logs(&msgs), msgs).await.unwrap()
    }

    #[tokio::test]
    async fn publish_echoes_contiguous_watermark() {
        let client = MemMailbox::<Msg>::new().client();

        let response = publish(
            &client,
            vec![msg(0, 'a', 0), msg(0, 'a', 1), msg(0, 'a', 3)],
        )
        .await;
        assert_eq!(response.watermark(&'a', &0), Some(1));

        let response = publish(&client, vec![msg(0, 'a', 2)]).await;
        assert_eq!(response.watermark(&'a', &0), Some(3));

        let response = publish(&client, vec![msg(1, 'b', 5)]).await;
        assert_eq!(response.watermark(&'b', &1), None);
    }

    pub type MsgTopic = u8;

    #[derive(Debug, PartialEq, Eq, Default)]
    struct Fetched {
        items: Vec<Msg>,
        missing: BTreeMap<(char, u8), Vec<SeqNum>>,
    }

    /// Fetch one topic, giving the heights of the `(author, log)` pairs.
    async fn fetch(
        client: &MemMailboxClient<Msg>,
        topic: MsgTopic,
        heights: &[(char, u8, SeqNum)],
    ) -> Fetched {
        let mut request = FetchRequest {
            topics: BTreeSet::from([topic]),
            heights: BTreeMap::new(),
        };
        for (author, log, seq) in heights {
            request
                .heights
                .entry(*author)
                .or_default()
                .insert(*log, *seq);
        }
        let response = client.fetch(request).await.unwrap();
        Fetched {
            items: response.items,
            missing: response
                .missing
                .into_iter()
                .flat_map(|(author, logs)| {
                    logs.into_iter()
                        .map(move |(log, seqs)| ((author, log), seqs))
                })
                .collect(),
        }
    }

    fn m(topic: MsgTopic, author: char, seq: SeqNum) -> Msg {
        Msg { topic, author, seq }
    }

    fn mm(topic: MsgTopic, author: char, r: std::ops::Range<SeqNum>) -> Vec<Msg> {
        r.map(|i| m(topic, author, i)).collect()
    }

    #[tokio::test]
    async fn test_mem_initial_sync() {
        let mailbox = MemMailbox::<Msg>::new();
        let client = mailbox.client();

        let a = 'a';
        let t = 11;

        assert_eq!(
            fetch(&client, t, &[(a, t, 1)]).await,
            Fetched {
                items: vec![],
                missing: BTreeMap::from([((a, t), vec![0, 1])]),
            }
        );

        publish(&client, mm(t, a, 0..2)).await;

        assert_eq!(
            fetch(&client, t, &[]).await,
            Fetched {
                items: mm(t, a, 0..2),
                missing: BTreeMap::new(),
            }
        );
    }

    #[tokio::test]
    async fn test_mem_mailbox() {
        let mailbox = MemMailbox::<Msg>::new();
        let client = mailbox.client();

        let a = '.';
        let tt: [MsgTopic; 3] = [11, 22, 33];

        let empty = Fetched::default();
        assert_eq!(fetch(&client, tt[0], &[]).await, empty);
        assert_eq!(fetch(&client, tt[1], &[]).await, empty);
        assert_eq!(fetch(&client, tt[2], &[]).await, empty);

        // only the first half
        publish(&client, mm(tt[0], a, 0..2)).await;
        // only the last half
        publish(&client, mm(tt[1], a, 2..4)).await;
        // both halves
        publish(&client, mm(tt[2], a, 0..4)).await;

        assert_eq!(
            fetch(&client, tt[0], &[(a, tt[0], 3)]).await,
            Fetched {
                items: vec![],
                missing: BTreeMap::from([((a, tt[0]), vec![2, 3])]),
            }
        );
        assert_eq!(
            fetch(&client, tt[1], &[(a, tt[1], 3)]).await,
            Fetched {
                items: vec![],
                missing: BTreeMap::from([((a, tt[1]), vec![0, 1])]),
            }
        );
        assert_eq!(fetch(&client, tt[2], &[(a, tt[2], 3)]).await, empty);

        assert_eq!(
            fetch(&client, tt[0], &[(a, tt[0], 4)]).await,
            Fetched {
                items: vec![],
                missing: BTreeMap::from([((a, tt[0]), vec![2, 3, 4])]),
            }
        );
        assert_eq!(
            fetch(&client, tt[1], &[(a, tt[1], 4)]).await,
            Fetched {
                items: vec![],
                missing: BTreeMap::from([((a, tt[1]), vec![0, 1, 4])]),
            }
        );
        assert_eq!(
            fetch(&client, tt[2], &[(a, tt[2], 4)]).await,
            Fetched {
                items: vec![],
                missing: BTreeMap::from([((a, tt[2]), vec![4])]),
            }
        );

        publish(&client, vec![m(tt[0], a, 4)]).await;
        publish(&client, vec![m(tt[1], a, 4)]).await;
        publish(&client, vec![m(tt[2], a, 4)]).await;
        assert_eq!(
            fetch(&client, tt[0], &[(a, tt[0], 3)]).await,
            Fetched {
                items: vec![m(tt[0], a, 4)],
                missing: BTreeMap::from([((a, tt[0]), vec![2, 3])]),
            }
        );
        assert_eq!(
            fetch(&client, tt[1], &[(a, tt[1], 3)]).await,
            Fetched {
                items: vec![m(tt[1], a, 4)],
                missing: BTreeMap::from([((a, tt[1]), vec![0, 1])]),
            }
        );
    }

    /// A log known under another topic is neither re-sent nor reported
    /// missing when a topic it is newly associated with is fetched.
    #[tokio::test]
    async fn test_mem_shared_log() {
        let mailbox = MemMailbox::<Msg>::new();
        let client = mailbox.client();

        publish(&client, mm(1, 'a', 0..3)).await;

        // Associate log 1 with topic 2 as well, without publishing anything.
        let shared: TopicLogs<Msg> =
            BTreeMap::from([(2, BTreeMap::from([('a', BTreeSet::from([1u8]))]))]);
        client.publish(shared, vec![]).await.unwrap();

        let fetched = fetch(&client, 2, &[('a', 1, 2)]).await;
        assert_eq!(fetched, Fetched::default());

        let fetched = fetch(&client, 2, &[]).await;
        assert_eq!(fetched.items, mm(1, 'a', 0..3));
    }
}

/// Highest `n` such that seqs `0..=n` are all present.
fn contiguous_watermark<Item>(log: &BTreeMap<SeqNum, Item>) -> Option<SeqNum> {
    let mut watermark = None;
    for &seq in log.keys() {
        if seq == watermark.map_or(0, |w| w + 1) {
            watermark = Some(seq);
        } else {
            break;
        }
    }
    watermark
}
