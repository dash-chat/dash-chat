use serde::{Deserialize, Serialize};

use crate::{MailboxItem, store::MailboxStore};

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, derive_more::Debug)]
#[debug("Msg({author} {seq})")]
pub struct Msg {
    pub topic: u8,
    pub author: char,
    pub seq: u64,
}

impl MailboxItem for Msg {
    type Author = char;
    type Hash = (char, u64);
    type Topic = u8;

    fn hash(&self) -> Self::Hash {
        (self.author, self.seq)
    }
    fn author(&self) -> Self::Author {
        self.author
    }
    fn seq_num(&self) -> u64 {
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
        _from: u64,
    ) -> Result<Option<Vec<Msg>>, anyhow::Error> {
        Ok(None)
    }
    async fn get_log_heights(&self, _topic: &u8) -> Result<Vec<(char, u64)>, anyhow::Error> {
        Ok(vec![])
    }
}
