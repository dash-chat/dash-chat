pub use crate::mem_server::MemMailbox;

use super::*;

use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    sync::Arc,
};

use tokio::sync::RwLock;

/// A client for the in-memory mailbox server.
/// This client is stateful, so all requests for a node should go through a single client
/// instance. State is shared between all cloned copies of this.
#[derive(Clone)]
pub struct MemMailboxClient<Item: MailboxItem> {
    pub(crate) mailbox: MemMailbox<Item>,
    pub(crate) subscribed_topics: Arc<RwLock<BTreeSet<Item::Topic>>>,
}

impl<Item: MailboxItem> MemMailboxClient<Item> {
    pub async fn subscribed_topics(&self) -> BTreeSet<Item::Topic> {
        self.subscribed_topics.read().await.clone()
    }
}

#[async_trait::async_trait]
impl<Item: MailboxItem> BlobStore for MemMailboxClient<Item> {
    async fn has_blob(&self, hash: BlobHash) -> anyhow::Result<bool> {
        let store = self.mailbox.blobs.read().await;
        Ok(store.contains_key(&hash))
    }

    async fn get_blob(&self, hash: BlobHash) -> anyhow::Result<Option<Blob>> {
        let store = self.mailbox.blobs.read().await;
        let blob = store.get(&hash).cloned();
        Ok(blob)
    }

    async fn store_blob(&self, blob: Blob) -> anyhow::Result<BlobHash> {
        let mut store = self.mailbox.blobs.write().await;
        let hash = blob.to_hash();
        store.insert(hash.clone(), blob);
        Ok(hash)
    }
}
#[async_trait::async_trait]
impl<Item: MailboxItem> MailboxClient<Item> for MemMailboxClient<Item>
where
    Item::Topic: OptionalItemTraits,
{
    fn id(&self) -> MailboxId {
        self.mailbox.id.clone()
    }

    async fn publish(&self, ops: Vec<Item>) -> Result<(), anyhow::Error> {
        let mut store = self.mailbox.ops.write().await;
        // ops.entry(topic).or_insert_with(Vec::new).push(op.into());
        for op in ops {
            let author = op.author();
            let seq_num = op.seq_num();
            let topic = op.topic();
            #[cfg(feature = "named-id")]
            tracing::info!(topic = ?topic.renamed(), hash = ?op.hash().renamed(), "publishing mailbox operation");
            store
                .entry(topic)
                .or_default()
                .entry(author)
                .or_default()
                .insert(seq_num, op);
        }
        Ok(())
    }

    async fn fetch(&self, request: FetchRequest<Item>) -> anyhow::Result<FetchResponse<Item>> {
        let mailbox_ops = self.mailbox.ops.read().await;

        let mut response = BTreeMap::new();

        for (topic, provided_authors) in request.0.into_iter() {
            let _empty_map = HashMap::new();
            let mailbox_authors = mailbox_ops.get(&topic).unwrap_or(&_empty_map);

            let mut new = vec![];
            let mut missing = HashMap::new();
            let all_authors = mailbox_authors
                .keys()
                .cloned()
                .chain(provided_authors.keys().cloned())
                .collect::<HashSet<_>>();
            for author in all_authors {
                let mut gaps = vec![];
                let mailbox_slots = mailbox_authors.get(&author).cloned();
                let provided_height = provided_authors.get(&author).cloned();

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
                                    new.push(op.clone());
                                }
                            } else {
                                gaps.push(i);
                            }
                        }
                    }
                    (Some(mailbox_slots), None) => {
                        new.extend(mailbox_slots.values().cloned());
                    }
                    (None, Some(provided_height)) => {
                        gaps.extend(0..=provided_height);
                    }
                    (None, None) => {
                        // empty mailbox, empty node store, return nothing...
                    }
                }
                if !gaps.is_empty() {
                    missing.insert(author, gaps);
                }
            }

            #[cfg(feature = "named-id")]
            tracing::info!(
                topic = ?topic.renamed(),
                num = new.len(),
                hashes = ?new.iter().map(|op: &Item| op.hash().renamed()).collect::<Vec<_>>(),
                "fetching mailbox items"
            );

            response.insert(
                topic,
                FetchTopicResponse {
                    items: new,
                    missing,
                },
            );
        }

        Ok(FetchResponse(response))
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[derive(Clone, PartialEq, Eq, Serialize, Deserialize, derive_more::Debug)]
    #[debug("Msg({author} {seq})")]
    struct Msg {
        topic: MsgTopic,
        author: char,
        seq: u64,
    }

    pub type MsgTopic = u8;

    impl MailboxItem for Msg {
        type Hash = (Self::Author, u64);
        type Author = char;
        type Topic = MsgTopic;

        fn hash(&self) -> Self::Hash {
            (self.author, self.seq)
        }

        fn author(&self) -> Self::Author {
            self.author.clone()
        }

        fn seq_num(&self) -> u64 {
            self.seq
        }

        fn topic(&self) -> Self::Topic {
            self.topic
        }
    }

    async fn fetch(
        client: &MemMailboxClient<Msg>,
        topic: MsgTopic,
        authors: &[(char, u64)],
    ) -> anyhow::Result<FetchTopicResponse<Msg>> {
        let FetchResponse(mut r) = client.fetch(r(topic, authors)).await?;
        let rr = r.remove(&topic).unwrap();
        assert!(r.is_empty());
        Ok(rr)
    }

    fn r(topic: MsgTopic, authors: &[(char, u64)]) -> FetchRequest<Msg> {
        FetchRequest(BTreeMap::from([(
            topic,
            BTreeMap::from_iter(authors.into_iter().cloned()),
        )]))
    }

    fn m(topic: MsgTopic, author: char, seq: u64) -> Msg {
        Msg { topic, author, seq }
    }

    fn mm(topic: MsgTopic, author: char, r: std::ops::Range<u64>) -> Vec<Msg> {
        r.map(|i| m(topic, author, i)).collect()
    }

    #[tokio::test]
    async fn test_mem_initial_sync() {
        let mailbox = MemMailbox::<Msg>::new();
        let client = mailbox.client();

        let a = 'a';
        let t = 11;

        assert_eq!(
            fetch(&client, t, &[(a, 1)]).await.unwrap(),
            FetchTopicResponse {
                items: vec![],
                missing: HashMap::from([(a, vec![0, 1])]),
            }
        );

        client.publish(mm(t, a, 0..2)).await.unwrap();

        assert_eq!(
            fetch(&client, t, &[]).await.unwrap(),
            FetchTopicResponse {
                items: mm(t, a, 0..2),
                missing: HashMap::new(),
            }
        );
    }

    #[tokio::test]
    async fn test_mem_blobs() {
        let mailbox = MemMailbox::<Msg>::new();
        let client1 = mailbox.client();
        let client2 = mailbox.client();

        let blob = Blob::from_static(b"hello");

        client1.store_blob(blob.clone()).await.unwrap();
        let fetched = client2.get_blob(blob.to_hash()).await.unwrap().unwrap();
        assert_eq!(fetched, blob);
    }

    #[tokio::test]
    async fn test_mem_mailbox() {
        let mailbox = MemMailbox::<Msg>::new();
        let client = mailbox.client();

        let a = '.';
        let tt: [MsgTopic; 3] = [11, 22, 33];

        let empty = FetchTopicResponse {
            items: vec![],
            missing: HashMap::new(),
        };
        assert_eq!(fetch(&client, tt[0], &[]).await.unwrap(), empty);
        assert_eq!(fetch(&client, tt[1], &[]).await.unwrap(), empty);
        assert_eq!(fetch(&client, tt[2], &[]).await.unwrap(), empty);

        // only the first half
        client.publish(mm(tt[0], a, 0..2)).await.unwrap();
        // only the last half
        client.publish(mm(tt[1], a, 2..4)).await.unwrap();
        // both halves
        client.publish(mm(tt[2], a, 0..4)).await.unwrap();

        assert_eq!(
            fetch(&client, tt[0], &[(a, 3)]).await.unwrap(),
            FetchTopicResponse {
                items: vec![],
                missing: HashMap::from([(a, vec![2, 3])]),
            }
        );
        assert_eq!(
            fetch(&client, tt[1], &[(a, 3)]).await.unwrap(),
            FetchTopicResponse {
                items: vec![],
                missing: HashMap::from([(a, vec![0, 1])]),
            }
        );
        assert_eq!(fetch(&client, tt[2], &[(a, 3)]).await.unwrap(), empty);

        assert_eq!(
            fetch(&client, tt[0], &[(a, 4)]).await.unwrap(),
            FetchTopicResponse {
                items: vec![],
                missing: HashMap::from([(a, vec![2, 3, 4])]),
            }
        );
        assert_eq!(
            fetch(&client, tt[1], &[(a, 4)]).await.unwrap(),
            FetchTopicResponse {
                items: vec![],
                missing: HashMap::from([(a, vec![0, 1, 4])]),
            }
        );
        assert_eq!(
            fetch(&client, tt[2], &[(a, 4)]).await.unwrap(),
            FetchTopicResponse {
                items: vec![],
                missing: HashMap::from([(a, vec![4])]),
            }
        );

        client.publish(vec![m(tt[0], a, 4)]).await.unwrap();
        client.publish(vec![m(tt[1], a, 4)]).await.unwrap();
        client.publish(vec![m(tt[2], a, 4)]).await.unwrap();
        assert_eq!(
            fetch(&client, tt[0], &[(a, 3)]).await.unwrap(),
            FetchTopicResponse {
                items: vec![m(tt[0], a, 4)],
                missing: HashMap::from([(a, vec![2, 3])]),
            }
        );
        assert_eq!(
            fetch(&client, tt[1], &[(a, 3)]).await.unwrap(),
            FetchTopicResponse {
                items: vec![m(tt[1], a, 4)],
                missing: HashMap::from([(a, vec![0, 1])]),
            }
        );
    }
}
