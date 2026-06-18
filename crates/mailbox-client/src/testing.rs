use serde::{Deserialize, Serialize};

use crate::{MailboxItem, store::MailboxStore};

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, derive_more::Debug)]
#[debug("Msg({author} {seq})")]
pub struct Msg {
    pub log_id: u8,
    pub author: char,
    pub seq: u64,
}

impl MailboxItem for Msg {
    type Author = char;
    type Hash = (char, u64);
    type LogId = u8;

    fn hash(&self) -> Self::Hash {
        (self.author, self.seq)
    }
    fn author(&self) -> Self::Author {
        self.author
    }
    fn seq_num(&self) -> u64 {
        self.seq
    }
    fn log_id(&self) -> Self::LogId {
        self.log_id
    }
}

#[derive(Clone)]
pub struct DummyStore;

#[async_trait::async_trait]
impl MailboxStore<Msg> for DummyStore {
    async fn get_log(
        &self,
        _author: &char,
        _log_id: &u8,
        _from: u64,
    ) -> Result<Option<Vec<Msg>>, anyhow::Error> {
        Ok(None)
    }
    async fn get_log_heights(&self, _log_id: &u8) -> Result<Vec<(char, u64)>, anyhow::Error> {
        Ok(vec![])
    }
}
