use crate::mem::MemMailboxClient;

use super::*;

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    sync::{Arc, atomic::AtomicBool},
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
    pub(crate) blobs: Arc<RwLock<HashMap<OpaqHash, Opaq>>>,
    pub(crate) running: Arc<AtomicBool>,
}

impl<Item: MailboxItem> MemMailbox<Item> {
    pub fn new() -> Self {
        Self {
            id: nanoid::nanoid!(),
            ops: Arc::new(RwLock::new(HashMap::new())),
            blobs: Arc::new(RwLock::new(HashMap::new())),
            running: Arc::new(AtomicBool::new(true)),
        }
    }

    pub fn client(&self) -> MemMailboxClient<Item> {
        MemMailboxClient {
            mailbox: self.clone(),
            subscribed_topics: Arc::new(RwLock::new(BTreeSet::new())),
        }
    }

    pub fn stop(&self) {
        self.running
            .store(false, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn start(&self) {
        self.running
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }
}
