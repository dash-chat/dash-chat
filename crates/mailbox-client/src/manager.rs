use crate::store::MailboxStore;
use tokio::time::Instant;

use super::*;

#[derive(Clone, Debug)]
pub struct MailboxesConfig {
    /// Polling interval for healthy mailboxes
    pub active_interval: Duration,
    /// Polling interval after recent errors
    pub degraded_interval: Duration,
    /// Polling interval after repeated failures
    pub stopped_interval: Duration,
    /// Delay between consecutive mailbox polls
    pub between_polls_delay: Duration,
    /// Number of consecutive errors to enter Degraded status
    pub degraded_threshold: u32,
    /// Number of consecutive errors to enter Stopped status
    pub stopped_threshold: u32,
}

impl Default for MailboxesConfig {
    fn default() -> Self {
        Self {
            active_interval: Duration::from_secs(5),
            degraded_interval: Duration::from_secs(30),
            stopped_interval: Duration::from_secs(300),
            between_polls_delay: Duration::from_millis(500),
            degraded_threshold: 2,
            stopped_threshold: 3,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncStatus {
    Active,
    Degraded,
    Stopped,
}

impl SyncStatus {
    fn interval(&self, config: &MailboxesConfig) -> Duration {
        match self {
            SyncStatus::Active => config.active_interval,
            SyncStatus::Degraded => config.degraded_interval,
            SyncStatus::Stopped => config.stopped_interval,
        }
    }
}

#[derive(Clone, Debug)]
struct MailboxTracker {
    status: SyncStatus,
    consecutive_errors: u32,
    next_poll: Instant,
}

impl MailboxTracker {
    fn new() -> Self {
        Self {
            status: SyncStatus::Active,
            consecutive_errors: 0,
            next_poll: Instant::now(),
        }
    }

    fn record_success(&mut self, config: &MailboxesConfig) {
        self.consecutive_errors = 0;
        self.status = SyncStatus::Active;
        self.next_poll = Instant::now() + config.active_interval;
    }

    fn record_error(&mut self, config: &MailboxesConfig) {
        self.consecutive_errors += 1;
        self.status = if self.consecutive_errors >= config.stopped_threshold {
            SyncStatus::Stopped
        } else if self.consecutive_errors >= config.degraded_threshold {
            SyncStatus::Degraded
        } else {
            self.status
        };
        self.next_poll = Instant::now() + self.status.interval(config);
    }

    fn reschedule(&mut self, config: &MailboxesConfig) {
        self.next_poll = Instant::now() + self.status.interval(config);
    }
}

struct TrackedMailbox<Item: MailboxItem> {
    client: Arc<dyn MailboxClient<Item>>,
    tracker: MailboxTracker,
}

impl<Item: MailboxItem> Clone for TrackedMailbox<Item> {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            tracker: self.tracker.clone(),
        }
    }
}

#[derive(Clone)]
pub struct Mailboxes<Item, Store>
where
    Item: MailboxItem,
    Store: MailboxStore<Item>,
{
    mailboxes: Arc<Mutex<BTreeMap<MailboxId, TrackedMailbox<Item>>>>,
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

    pub async fn register(&self, mailbox: impl MailboxClient<Item>) {
        // TODO: check for existing mailbox with different ID but same "URL" (which is currently abstracted away and inaccessible here, darn)
        // TODO: make the ID come from the mailbox server itself, e.g. for mDNS discovery the ID is set by the mDNS service, but multiple services could point to the same actual mailbox state.
        let id = mailbox.id();
        let tracked = TrackedMailbox {
            client: Arc::new(mailbox),
            tracker: MailboxTracker::new(),
        };
        let existing = self.mailboxes.lock().await.insert(id.clone(), tracked);
        if existing.is_some() {
            // TODO: potentially track multiple clients for a single mailbox ID, e.g. multiple mDNS discovered addresses for the same node
            // TODO: at least, make sure the URL being replaced is "better" than the previous one, i.e. ipv4 instead of ipv6
            tracing::warn!("overwriting existing mailbox for {id}");
        }
        self.trigger_sync();
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
    ) -> Result<Option<mpsc::Receiver<Item>>, anyhow::Error> {
        #[cfg(feature = "named-id")]
        tracing::info!(topic = ?topic.renamed(), "subscribing to topic");

        let mut tt = self.topics.lock().await;
        if tt.contains_key(&topic) {
            return Ok(None);
        }
        let (tx, rx) = mpsc::channel(100);
        tt.insert(topic, tx);
        Ok(Some(rx))
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
                loop {
                    let next = manager.find_next_due().await;

                    match next {
                        None => {
                            // No mailboxes registered, wait for a trigger
                            match trigger_rx.recv().await {
                                Some(()) => continue,
                                None => break,
                            }
                        }
                        Some((id, wait)) => {
                            if !wait.is_zero() {
                                // Sleep until the next mailbox is due, or a trigger wakes us
                                match tokio::time::timeout(wait, trigger_rx.recv()).await {
                                    Ok(None) => break,   // channel closed
                                    Ok(Some(())) => {},  // triggered early, re-evaluate
                                    Err(_) => {},        // timeout elapsed
                                }
                                // Re-evaluate which mailbox is actually due now
                                continue;
                            }

                            manager.poll_mailbox(&id).await;
                            tokio::time::sleep(manager.config.between_polls_delay).await;
                        }
                    }
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

    async fn find_next_due(&self) -> Option<(MailboxId, Duration)> {
        let mm = self.mailboxes.lock().await;
        if mm.is_empty() {
            return None;
        }

        let now = Instant::now();
        let (id, tracked) = mm
            .iter()
            .min_by_key(|(_, t)| t.tracker.next_poll)
            .unwrap();

        let wait = if tracked.tracker.next_poll <= now {
            Duration::ZERO
        } else {
            tracked.tracker.next_poll - now
        };

        Some((id.clone(), wait))
    }

    async fn poll_mailbox(&self, id: &MailboxId) {
        let client = {
            let mm = self.mailboxes.lock().await;
            match mm.get(id) {
                Some(tracked) => tracked.client.clone(),
                None => return,
            }
        };

        let topics = self.subscribed_topics().await;
        if topics.is_empty() {
            tracing::trace!("no topics subscribed, skipping poll for {id}");
            let mut mm = self.mailboxes.lock().await;
            if let Some(tracked) = mm.get_mut(id) {
                tracked.tracker.reschedule(&self.config);
            }
            return;
        }

        tracing::info!("polling mailbox {id}");
        let result = self.sync_topics(topics.into_iter(), client).await;

        let mut mm = self.mailboxes.lock().await;
        if let Some(tracked) = mm.get_mut(id) {
            match result {
                Ok(()) => tracked.tracker.record_success(&self.config),
                Err(err) => {
                    tracing::error!(?err, mailbox = %id, "mailbox sync error");
                    tracked.tracker.record_error(&self.config);
                    tracing::info!(
                        mailbox = %id,
                        status = ?tracked.tracker.status,
                        errors = tracked.tracker.consecutive_errors,
                        "mailbox status updated"
                    );
                }
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
