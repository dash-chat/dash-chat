use crate::store::MailboxStore;

use super::*;

#[derive(Clone, Debug)]
pub struct MailboxesConfig {
    pub success_interval: Duration,
    pub error_interval: Duration,
    pub min_interval: Duration,
}

impl Default for MailboxesConfig {
    fn default() -> Self {
        Self {
            success_interval: Duration::from_secs(5),
            error_interval: Duration::from_secs(15),
            min_interval: Duration::from_secs(1),
        }
    }
}

#[derive(Clone)]
pub struct Mailboxes<Item, Store>
where
    Item: MailboxItem,
    Store: MailboxStore<Item>,
{
    mailboxes: Arc<Mutex<Vec<Arc<dyn MailboxClient<Item>>>>>,
    topics: Arc<Mutex<HashMap<Item::Topic, mpsc::Sender<Item>>>>,
    store: Store,
    config: MailboxesConfig,
    trigger: mpsc::Sender<()>,
}

impl<Item, Store> Mailboxes<Item, Store>
where
    Item: MailboxItem,
    Store: MailboxStore<Item>,
    Item::Topic: OptionalItemTraits,
{
    fn new(store: Store, config: MailboxesConfig, trigger: mpsc::Sender<()>) -> Self {
        Self {
            mailboxes: Arc::new(Mutex::new(Default::default())),
            topics: Arc::new(Mutex::new(Default::default())),
            store,
            config,
            trigger,
        }
    }

    pub async fn add(&self, mailbox: impl MailboxClient<Item>) {
        self.mailboxes.lock().await.push(Arc::new(mailbox));
    }

    pub async fn clear(&self) {
        self.mailboxes.lock().await.clear();
    }

    pub async fn subscribed_topics(&self) -> BTreeSet<Item::Topic> {
        self.topics.lock().await.keys().cloned().collect()
    }

    pub fn trigger_sync(&self) {
        _ = self.trigger.try_send(());
    }

    pub async fn subscribe(
        &self,
        topic: Item::Topic,
    ) -> Result<mpsc::Receiver<Item>, anyhow::Error> {
        #[cfg(feature = "named-id")]
        tracing::info!(topic = ?topic.renamed(), "subscribing to topic");
        let (tx, rx) = mpsc::channel(100);
        self.topics.lock().await.insert(topic, tx);
        Ok(rx)
    }

    pub async fn unsubscribe(&self, topic: Item::Topic) -> Result<(), anyhow::Error> {
        #[cfg(feature = "named-id")]
        tracing::info!(topic = ?topic.renamed(), "unsubscribing from topic");
        self.topics.lock().await.remove(&topic);
        Ok(())
    }

    pub async fn spawn(store: Store, config: MailboxesConfig) -> Result<Self, anyhow::Error> {
        let (trigger_tx, mut trigger_rx) = mpsc::channel(1);
        let manager = Self::new(store, config, trigger_tx);
        let r = manager.clone();
        tokio::spawn(
            async move {
                let mut next_mailbox = 0;
                let mut next_interval;
                let mut last_iteration: tokio::time::Instant = tokio::time::Instant::now();
                loop {
                    (next_interval, next_mailbox) = manager.one_iteration(next_mailbox).await;

                    // The two match conditions are:
                    // - Ok(Some(())): a trigger was received
                    // - Err(_): the timeout elapsed
                    if let Ok(None) = tokio::time::timeout(next_interval, trigger_rx.recv()).await {
                        break;
                    }

                    // Ensure a minimum polling interval so we don't poll too often
                    let elapsed = last_iteration.elapsed();
                    if elapsed < manager.config.min_interval {
                        tokio::time::sleep(manager.config.min_interval - elapsed).await;
                    }

                    last_iteration = tokio::time::Instant::now();
                }

                #[allow(unused)]
                {
                    tracing::warn!("poll mailboxes loop exited");
                }
            }
            .instrument(tracing::info_span!("poll mailboxes")),
        );

        Ok(r)
    }

    async fn one_iteration(&self, mut mailbox_index: usize) -> (tokio::time::Duration, usize) {
        mailbox_index += 1;
        let mailbox = {
            let mm = self.mailboxes.lock().await;
            if mailbox_index >= mm.len() {
                mailbox_index = 0;
            }

            match mm.get(mailbox_index) {
                Some(mailbox) => mailbox.clone(),
                None => {
                    tracing::warn!("empty mailbox list, no mailbox to fetch from");
                    return (self.config.error_interval, mailbox_index);
                }
            }
        };
        tracing::trace!("polling mailbox {mailbox_index}");

        let topics = self.subscribed_topics().await;
        if topics.is_empty() {
            tracing::warn!("no topics subscribed, nothing to fetch this interval");
            return (self.config.error_interval, mailbox_index);
        }

        match self.sync_topics(topics.into_iter(), mailbox.clone()).await {
            Ok(()) => {
                return (self.config.success_interval, mailbox_index);
            }
            Err(err) => {
                tracing::error!(?err, "fetch mailbox error");
                return (self.config.error_interval, mailbox_index);
            }
        }
    }

    /// Immediately sync the given topics with the given mailbox:
    /// - Ensure all items held by the mailbox are fetched
    /// - Publish any items that the mailbox is missing to the mailbox
    pub async fn sync_topics(
        &self,
        topics: impl Iterator<Item = Item::Topic>,
        mailbox: Arc<dyn MailboxClient<Item>>,
    ) -> anyhow::Result<()> {
        let mut request = BTreeMap::new();
        for topic in topics {
            let heights =
                BTreeMap::from_iter(self.store.get_log_heights(&topic).await?.into_iter());
            request.insert(topic, heights);
        }

        let FetchResponse(response) = mailbox.fetch(FetchRequest(request)).await?;

        let mut ops_to_publish = vec![];
        for (topic, response) in response.into_iter() {
            let FetchTopicResponse { items, missing } = response;
            if items.is_empty() && missing.is_empty() {
                tracing::trace!(topic = ?topic, "Syncing with mailbox: nothing to do");
            } else {
                tracing::info!(
                    items = items.len(),
                    missing = missing.len(),
                    "fetched operations"
                );
            }

            let Some(sender) = self.topics.lock().await.get(&topic).cloned() else {
                #[cfg(feature = "named-id")]
                tracing::warn!(topic = ?topic.renamed(), "no sender for topic");
                continue;
            };

            for item in items {
                sender.send(item.into()).await?;
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
                    continue;
                };

                for seq in &seqs {
                    // The operations in the 0..lowest range are not included in the log vector,
                    // because `get_log()` is called with `lowest` as the starting point.
                    // Adjust the index to take this into account:
                    let index = seq - lowest;
                    if let Some(item) = log.get(index as usize) {
                        ops_to_publish.push(item.clone());
                    }
                }
            }
        }

        mailbox.publish(ops_to_publish).await?;

        Ok(())
    }
}
