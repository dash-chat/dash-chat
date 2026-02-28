use crate::mem::MemMailboxClient;

use super::*;

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    sync::Arc,
};

use tokio::sync::RwLock;

pub type MemMailboxLogs<Item> = HashMap<
    <Item as MailboxItem>::Topic,
    HashMap<<Item as MailboxItem>::Author, BTreeMap<u64, Item>>,
>;

#[derive(Clone)]
pub struct MemMailbox<Item: MailboxItem> {
    pub(crate) id: MailboxId,
    pub(crate) ops: Arc<RwLock<MemMailboxLogs<Item>>>,
    pub(crate) blobs: Arc<RwLock<HashMap<BlobHash, Opaq>>>,
}

impl<Item: MailboxItem> MemMailbox<Item> {
    pub fn new() -> Self {
        Self {
            id: nanoid::nanoid!(),
            ops: Arc::new(RwLock::new(HashMap::new())),
            blobs: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn client(&self) -> MemMailboxClient<Item> {
        MemMailboxClient {
            mailbox: self.clone(),
            subscribed_topics: Arc::new(RwLock::new(BTreeSet::new())),
        }
    }
}
