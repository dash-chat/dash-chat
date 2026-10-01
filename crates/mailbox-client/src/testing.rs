use serde::{Deserialize, Serialize};

use crate::{MailboxItem, SeqNum, store::MailboxStore};

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, derive_more::Debug)]
#[debug("Msg({author} {seq})")]
pub struct Msg {
    pub topic: u8,
    pub author: char,
    pub seq: SeqNum,
}

impl MailboxItem for Msg {
    type Author = char;
    type Hash = (char, SeqNum);
    type Topic = u8;

    fn hash(&self) -> Self::Hash {
        (self.author, self.seq)
    }
    fn author(&self) -> Self::Author {
        self.author
    }
    fn seq_num(&self) -> SeqNum {
        self.seq
    }
    fn topic(&self) -> Self::Topic {
        self.topic
    }
}

#[derive(Clone)]
pub struct DummyStore;

#[async_trait::async_trait]
impl MailboxStore<Msg> for DummyStore {
    async fn get_log(
        &self,
        _author: &char,
        _topic: &u8,
        _from: SeqNum,
    ) -> Result<Option<Vec<Msg>>, anyhow::Error> {
        Ok(None)
    }
    async fn get_log_heights(&self, _topic: &u8) -> Result<Vec<(char, SeqNum)>, anyhow::Error> {
        Ok(vec![])
    }
}

/// A store holding one complete log per (topic, author): seqs `0..=height`.
#[derive(Clone, Default)]
pub struct MemStore {
    heights: std::sync::Arc<std::sync::Mutex<std::collections::HashMap<(u8, char), SeqNum>>>,
}

impl MemStore {
    pub fn set_height(&self, topic: u8, author: char, height: SeqNum) {
        self.heights.lock().unwrap().insert((topic, author), height);
    }
}

#[async_trait::async_trait]
impl MailboxStore<Msg> for MemStore {
    async fn get_log(
        &self,
        author: &char,
        topic: &u8,
        from: SeqNum,
    ) -> Result<Option<Vec<Msg>>, anyhow::Error> {
        let Some(height) = self
            .heights
            .lock()
            .unwrap()
            .get(&(*topic, *author))
            .copied()
        else {
            return Ok(None);
        };
        Ok(Some(
            (from..=height)
                .map(|seq| Msg {
                    topic: *topic,
                    author: *author,
                    seq,
                })
                .collect(),
        ))
    }

    async fn get_log_heights(&self, topic: &u8) -> Result<Vec<(char, SeqNum)>, anyhow::Error> {
        Ok(self
            .heights
            .lock()
            .unwrap()
            .iter()
            .filter(|((t, _), _)| t == topic)
            .map(|((_, a), h)| (*a, *h))
            .collect())
    }
}
