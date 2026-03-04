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
                                    Ok(None) => break, // channel closed
                                    Ok(Some(())) => {} // triggered early, re-evaluate
                                    Err(_) => {}       // timeout elapsed
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
        let (id, tracked) = mm.iter().min_by_key(|(_, t)| t.tracker.next_poll).unwrap();

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        mem::MemMailbox,
        testing::{DummyStore, Msg},
    };

    fn test_config() -> MailboxesConfig {
        MailboxesConfig {
            active_interval: Duration::from_secs(5),
            degraded_interval: Duration::from_secs(30),
            stopped_interval: Duration::from_secs(300),
            between_polls_delay: Duration::from_millis(500),
            degraded_threshold: 2,
            stopped_threshold: 3,
        }
    }

    /// Create a Mailboxes instance without spawning the background loop
    fn test_mailboxes(config: MailboxesConfig) -> Mailboxes<Msg, DummyStore> {
        let (trigger_tx, _trigger_rx) = mpsc::channel(1);
        Mailboxes::new(DummyStore, config, trigger_tx)
    }

    // -- MailboxTracker unit tests --

    #[tokio::test(start_paused = true)]
    async fn tracker_starts_active() {
        let tracker = MailboxTracker::new();
        assert_eq!(tracker.status, SyncStatus::Active);
        assert_eq!(tracker.consecutive_errors, 0);
    }

    #[tokio::test(start_paused = true)]
    async fn tracker_success_resets_to_active() {
        let config = test_config();
        let mut tracker = MailboxTracker::new();

        // Accumulate some errors first
        tracker.record_error(&config);
        tracker.record_error(&config);
        assert_eq!(tracker.status, SyncStatus::Degraded);

        // Success resets everything
        tracker.record_success(&config);
        assert_eq!(tracker.status, SyncStatus::Active);
        assert_eq!(tracker.consecutive_errors, 0);
    }

    #[tokio::test(start_paused = true)]
    async fn tracker_error_transitions() {
        let config = test_config();
        let mut tracker = MailboxTracker::new();

        // 1 error: still Active
        tracker.record_error(&config);
        assert_eq!(tracker.status, SyncStatus::Active);
        assert_eq!(tracker.consecutive_errors, 1);

        // 2 errors: Degraded
        tracker.record_error(&config);
        assert_eq!(tracker.status, SyncStatus::Degraded);
        assert_eq!(tracker.consecutive_errors, 2);

        // 3 errors: Stopped
        tracker.record_error(&config);
        assert_eq!(tracker.status, SyncStatus::Stopped);
        assert_eq!(tracker.consecutive_errors, 3);

        // More errors: stays Stopped
        tracker.record_error(&config);
        assert_eq!(tracker.status, SyncStatus::Stopped);
        assert_eq!(tracker.consecutive_errors, 4);
    }

    #[tokio::test(start_paused = true)]
    async fn tracker_next_poll_after_success() {
        let config = test_config();
        let mut tracker = MailboxTracker::new();

        let before = Instant::now();
        tracker.record_success(&config);
        let expected = before + config.active_interval;

        assert_eq!(tracker.next_poll, expected);
    }

    #[tokio::test(start_paused = true)]
    async fn tracker_next_poll_after_errors() {
        let config = test_config();
        let mut tracker = MailboxTracker::new();

        // First error: still Active interval
        tracker.record_error(&config);
        let expected = Instant::now() + config.active_interval;
        assert_eq!(tracker.next_poll, expected);

        tokio::time::advance(Duration::from_secs(1)).await;

        // Second error: Degraded interval
        tracker.record_error(&config);
        let expected = Instant::now() + config.degraded_interval;
        assert_eq!(tracker.next_poll, expected);

        tokio::time::advance(Duration::from_secs(1)).await;

        // Third error: Stopped interval
        tracker.record_error(&config);
        let expected = Instant::now() + config.stopped_interval;
        assert_eq!(tracker.next_poll, expected);
    }

    // -- find_next_due tests --

    #[tokio::test(start_paused = true)]
    async fn find_next_due_empty() {
        let mgr = test_mailboxes(test_config());
        assert!(mgr.find_next_due().await.is_none());
    }

    #[tokio::test(start_paused = true)]
    async fn find_next_due_single_new_mailbox() {
        let mgr = test_mailboxes(test_config());
        let mb = MemMailbox::<Msg>::new();
        let client = mb.client();
        let id = client.id();
        mgr.register(client).await;

        // Newly registered mailbox should be due immediately
        let (found_id, wait) = mgr.find_next_due().await.unwrap();
        assert_eq!(found_id, id);
        assert_eq!(wait, Duration::ZERO);
    }

    #[tokio::test(start_paused = true)]
    async fn find_next_due_picks_earliest() {
        let config = test_config();
        let mgr = test_mailboxes(config.clone());

        // Register first mailbox
        let mb1 = MemMailbox::<Msg>::new();
        let c1 = mb1.client();
        let id1 = c1.id();
        mgr.register(c1).await;

        // Simulate a successful poll so it gets scheduled into the future
        {
            let mut mm = mgr.mailboxes.lock().await;
            mm.get_mut(&id1).unwrap().tracker.record_success(&config);
        }

        // Advance time a bit
        tokio::time::advance(Duration::from_secs(2)).await;

        // Register second mailbox (will be due immediately)
        let mb2 = MemMailbox::<Msg>::new();
        let c2 = mb2.client();
        let id2 = c2.id();
        mgr.register(c2).await;

        // Second mailbox should be picked (it's due now)
        let (found_id, wait) = mgr.find_next_due().await.unwrap();
        assert_eq!(found_id, id2);
        assert_eq!(wait, Duration::ZERO);
    }

    #[tokio::test(start_paused = true)]
    async fn find_next_due_returns_wait_duration() {
        let config = test_config();
        let mgr = test_mailboxes(config.clone());

        let mb = MemMailbox::<Msg>::new();
        let client = mb.client();
        let id = client.id();
        mgr.register(client).await;

        // Simulate a successful poll
        {
            let mut mm = mgr.mailboxes.lock().await;
            mm.get_mut(&id).unwrap().tracker.record_success(&config);
        }

        // Should need to wait ~active_interval
        let (found_id, wait) = mgr.find_next_due().await.unwrap();
        assert_eq!(found_id, id);
        assert_eq!(wait, config.active_interval);

        // Advance partway
        tokio::time::advance(Duration::from_secs(3)).await;

        let (_found_id, wait) = mgr.find_next_due().await.unwrap();
        assert_eq!(wait, Duration::from_secs(2));
    }

    #[tokio::test(start_paused = true)]
    async fn find_next_due_respects_degraded_interval() {
        let config = test_config();
        let mgr = test_mailboxes(config.clone());

        let mb = MemMailbox::<Msg>::new();
        let client = mb.client();
        let id = client.id();
        mgr.register(client).await;

        // Simulate reaching Degraded status
        {
            let mut mm = mgr.mailboxes.lock().await;
            let tracked = mm.get_mut(&id).unwrap();
            tracked.tracker.record_error(&config); // 1 error: Active
            tracked.tracker.record_error(&config); // 2 errors: Degraded
        }

        let (_found_id, wait) = mgr.find_next_due().await.unwrap();
        assert_eq!(wait, config.degraded_interval);
    }

    #[tokio::test(start_paused = true)]
    async fn find_next_due_respects_stopped_interval() {
        let config = test_config();
        let mgr = test_mailboxes(config.clone());

        let mb = MemMailbox::<Msg>::new();
        let client = mb.client();
        let id = client.id();
        mgr.register(client).await;

        // Simulate reaching Stopped status
        {
            let mut mm = mgr.mailboxes.lock().await;
            let tracked = mm.get_mut(&id).unwrap();
            tracked.tracker.record_error(&config);
            tracked.tracker.record_error(&config);
            tracked.tracker.record_error(&config);
            assert_eq!(tracked.tracker.status, SyncStatus::Stopped);
        }

        let (_found_id, wait) = mgr.find_next_due().await.unwrap();
        assert_eq!(wait, config.stopped_interval);
    }

    #[tokio::test(start_paused = true)]
    async fn find_next_due_overdue_returns_zero() {
        let config = test_config();
        let mgr = test_mailboxes(config.clone());

        let mb = MemMailbox::<Msg>::new();
        let client = mb.client();
        let id = client.id();
        mgr.register(client).await;

        // Schedule into the future
        {
            let mut mm = mgr.mailboxes.lock().await;
            mm.get_mut(&id).unwrap().tracker.record_success(&config);
        }

        // Advance past the interval
        tokio::time::advance(config.active_interval + Duration::from_secs(1)).await;

        let (_found_id, wait) = mgr.find_next_due().await.unwrap();
        assert_eq!(wait, Duration::ZERO);
    }

    #[tokio::test(start_paused = true)]
    async fn find_next_due_multiple_different_statuses() {
        let config = test_config();
        let mgr = test_mailboxes(config.clone());

        // Register three mailboxes
        let mb_a = MemMailbox::<Msg>::new();
        let ca = mb_a.client();
        let id_a = ca.id();
        mgr.register(ca).await;

        let mb_b = MemMailbox::<Msg>::new();
        let cb = mb_b.client();
        let id_b = cb.id();
        mgr.register(cb).await;

        let mb_c = MemMailbox::<Msg>::new();
        let cc = mb_c.client();
        let id_c = cc.id();
        mgr.register(cc).await;

        // Set different statuses:
        // A: Active (success), B: Degraded, C: Stopped
        {
            let mut mm = mgr.mailboxes.lock().await;
            mm.get_mut(&id_a).unwrap().tracker.record_success(&config);
            let b = mm.get_mut(&id_b).unwrap();
            b.tracker.record_error(&config);
            b.tracker.record_error(&config);
            let c = mm.get_mut(&id_c).unwrap();
            c.tracker.record_error(&config);
            c.tracker.record_error(&config);
            c.tracker.record_error(&config);
        }

        // A has shortest interval (5s), should be picked first
        let (found_id, wait) = mgr.find_next_due().await.unwrap();
        assert_eq!(found_id, id_a);
        assert_eq!(wait, config.active_interval);

        // Advance past active interval but not degraded
        tokio::time::advance(config.active_interval + Duration::from_secs(1)).await;

        // A is now overdue
        let (found_id, wait) = mgr.find_next_due().await.unwrap();
        assert_eq!(found_id, id_a);
        assert_eq!(wait, Duration::ZERO);
    }

    #[tokio::test(start_paused = true)]
    async fn find_next_due_after_clear() {
        let mgr = test_mailboxes(test_config());

        let mb = MemMailbox::<Msg>::new();
        mgr.register(mb.client()).await;
        assert!(mgr.find_next_due().await.is_some());

        mgr.clear().await;
        assert!(mgr.find_next_due().await.is_none());
    }
}
