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
            active_interval: Duration::from_secs(2),
            degraded_interval: Duration::from_secs(5),
            stopped_interval: Duration::from_secs(10),
            between_polls_delay: Duration::from_millis(500),
            degraded_threshold: 5,
            stopped_threshold: 10,
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
        self.next_poll = Instant::now() + config.active_interval + config.between_polls_delay;
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
        self.next_poll = Instant::now() + self.status.interval(config) + config.between_polls_delay;
    }

    fn reschedule(&mut self, config: &MailboxesConfig) {
        self.next_poll = Instant::now() + self.status.interval(config) + config.between_polls_delay;
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
    use std::sync::atomic::{AtomicU32, Ordering};

    use super::*;
    use crate::{
        mem::MemMailbox,
        testing::{DummyStore, Msg},
    };

    /// A mailbox client that counts fetch() calls and optionally returns errors.
    struct TrackingClient {
        id: MailboxId,
        poll_count: Arc<AtomicU32>,
        should_fail: bool,
    }

    impl TrackingClient {
        fn new(should_fail: bool) -> (Self, Arc<AtomicU32>) {
            let poll_count = Arc::new(AtomicU32::new(0));
            let client = Self {
                id: nanoid::nanoid!(),
                poll_count: poll_count.clone(),
                should_fail,
            };
            (client, poll_count)
        }
    }

    #[async_trait::async_trait]
    impl MailboxClient<Msg> for TrackingClient {
        fn id(&self) -> MailboxId {
            self.id.clone()
        }
        async fn publish(&self, _ops: Vec<Msg>) -> Result<(), anyhow::Error> {
            Ok(())
        }
        async fn fetch(
            &self,
            _request: FetchRequest<Msg>,
        ) -> Result<FetchResponse<Msg>, anyhow::Error> {
            self.poll_count.fetch_add(1, Ordering::Relaxed);
            if self.should_fail {
                Err(anyhow::anyhow!("simulated failure"))
            } else {
                Ok(FetchResponse(BTreeMap::new()))
            }
        }
    }

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
        let expected = before + config.active_interval + config.between_polls_delay;

        assert_eq!(tracker.next_poll, expected);
    }

    #[tokio::test(start_paused = true)]
    async fn tracker_next_poll_after_errors() {
        let config = test_config();
        let mut tracker = MailboxTracker::new();
        let delay = config.between_polls_delay;

        // First error: still Active interval
        tracker.record_error(&config);
        let expected = Instant::now() + config.active_interval + delay;
        assert_eq!(tracker.next_poll, expected);

        tokio::time::advance(Duration::from_secs(1)).await;

        // Second error: Degraded interval
        tracker.record_error(&config);
        let expected = Instant::now() + config.degraded_interval + delay;
        assert_eq!(tracker.next_poll, expected);

        tokio::time::advance(Duration::from_secs(1)).await;

        // Third error: Stopped interval
        tracker.record_error(&config);
        let expected = Instant::now() + config.stopped_interval + delay;
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
        let delay = config.between_polls_delay;
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

        // Should need to wait active_interval + delay
        let (found_id, wait) = mgr.find_next_due().await.unwrap();
        assert_eq!(found_id, id);
        assert_eq!(wait, config.active_interval + delay);

        // Advance partway (3s into 5.5s total)
        tokio::time::advance(Duration::from_secs(3)).await;

        let (_found_id, wait) = mgr.find_next_due().await.unwrap();
        assert_eq!(
            wait,
            config.active_interval + delay - Duration::from_secs(3)
        );
    }

    #[tokio::test(start_paused = true)]
    async fn find_next_due_respects_degraded_interval() {
        let config = test_config();
        let delay = config.between_polls_delay;
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
        assert_eq!(wait, config.degraded_interval + delay);
    }

    #[tokio::test(start_paused = true)]
    async fn find_next_due_respects_stopped_interval() {
        let config = test_config();
        let delay = config.between_polls_delay;
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
        assert_eq!(wait, config.stopped_interval + delay);
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

        // Advance past interval + delay
        tokio::time::advance(
            config.active_interval + config.between_polls_delay + Duration::from_secs(1),
        )
        .await;

        let (_found_id, wait) = mgr.find_next_due().await.unwrap();
        assert_eq!(wait, Duration::ZERO);
    }

    #[tokio::test(start_paused = true)]
    async fn find_next_due_multiple_different_statuses() {
        let config = test_config();
        let delay = config.between_polls_delay;
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

        // A has shortest effective interval, should be picked first
        let (found_id, wait) = mgr.find_next_due().await.unwrap();
        assert_eq!(found_id, id_a);
        assert_eq!(wait, config.active_interval + delay);

        // Advance past active effective interval but not degraded
        tokio::time::advance(config.active_interval + delay + Duration::from_secs(1)).await;

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

    // -- Fairness test with full spawn loop --

    /// A mailbox client whose fetch() always returns an error.
    struct FailingClient {
        id: MailboxId,
        poll_count: Arc<AtomicU32>,
    }

    impl FailingClient {
        fn new() -> (Self, Arc<AtomicU32>) {
            let poll_count = Arc::new(AtomicU32::new(0));
            let client = Self {
                id: nanoid::nanoid!(),
                poll_count: poll_count.clone(),
            };
            (client, poll_count)
        }
    }

    #[async_trait::async_trait]
    impl MailboxClient<Msg> for FailingClient {
        fn id(&self) -> MailboxId {
            self.id.clone()
        }
        async fn publish(&self, _ops: Vec<Msg>) -> Result<(), anyhow::Error> {
            Ok(())
        }
        async fn fetch(
            &self,
            _request: FetchRequest<Msg>,
        ) -> Result<FetchResponse<Msg>, anyhow::Error> {
            self.poll_count.fetch_add(1, Ordering::Relaxed);
            Err(anyhow::anyhow!("simulated failure"))
        }
    }

    /// Assert all values in the slice deviate by at most `max_diff` from each other.
    fn assert_within_group(label: &str, polls: &[u32], max_diff: u32) {
        let min = *polls.iter().min().unwrap();
        let max = *polls.iter().max().unwrap();
        assert!(
            max - min <= max_diff,
            "{label}: within-group deviation too large: \
             min={min}, max={max}, diff={} (allowed {max_diff}), polls={polls:?}",
            max - min,
        );
    }

    #[tokio::test(start_paused = true)]
    async fn polling_fairness() {
        // 10 mailboxes: 5 active, 3 degraded, 2 stopped.
        //
        // The effective interval is `status_interval + between_polls_delay`.
        //   active:   1.0 + 0.2 = 1.2s
        //   degraded: 1.5 + 0.2 = 1.7s
        //   stopped:  2.0 + 0.2 = 2.2s
        //
        // The per-mailbox polling rate is proportional to 1/effective_interval,
        // so the expected ratios are:
        //   active/stopped  = 2.2 / 1.2 ≈ 1.83
        //   active/degraded = 1.7 / 1.2 ≈ 1.42
        let config = MailboxesConfig {
            active_interval: Duration::from_secs(1),
            degraded_interval: Duration::from_millis(1500),
            stopped_interval: Duration::from_secs(2),
            between_polls_delay: Duration::from_millis(200),
            // One error → Degraded. Stopped threshold set very high so
            // degraded clients never naturally transition to Stopped
            // during the test; stopped clients are forced manually.
            degraded_threshold: 1,
            stopped_threshold: 1000,
        };

        let mgr = Mailboxes::<Msg, DummyStore>::spawn(DummyStore, config)
            .await
            .unwrap();

        // Subscribe to a topic so poll_mailbox actually calls sync_topics
        let _rx = mgr.subscribe(0u8).await.unwrap();

        let mut active_counts = vec![];
        let mut degraded_counts = vec![];
        let mut stopped_counts = vec![];

        // 5 active (always succeed)
        for _ in 0..5 {
            let (client, count) = TrackingClient::new(false);
            mgr.register(client).await;
            active_counts.push(count);
        }

        // 3 degraded: fail once → degraded_threshold=1 puts them into Degraded.
        // To get exactly Degraded (not Stopped), we register failing clients
        // and then after 1 error manually cap them at Degraded by swapping to
        // a succeeding client. Instead, simpler: use FailingClient (always fails),
        // but set thresholds so 1 error = Degraded and 2 errors = Stopped.
        // After the first poll they'll be Degraded. After the second they'll be
        // Stopped — which we don't want.
        //
        // Solution: use TrackingClient(should_fail=true) but after the initial
        // poll transitions them to Degraded, subsequent polls keep adding errors
        // and they'll become Stopped after 2 errors. So we need stopped_threshold
        // high enough that they stay Degraded throughout the test.
        //
        // Actually, let's use a different approach: separate configs won't work
        // since config is shared. Instead, we'll use FailingClient for both
        // degraded and stopped, but set thresholds so that after 1 error = Degraded
        // and after many errors (say 100) = Stopped. Then the "stopped" group
        // uses a client that we manually force into Stopped status.
        //
        // Simplest: just manually set tracker status after registration.

        // 3 degraded + 2 stopped: all use FailingClient, we manually set status
        let mut degraded_ids = vec![];
        for _ in 0..3 {
            let (client, count) = FailingClient::new();
            let id = client.id.clone();
            mgr.register(client).await;
            degraded_counts.push(count);
            degraded_ids.push(id);
        }
        let mut stopped_ids = vec![];
        for _ in 0..2 {
            let (client, count) = FailingClient::new();
            let id = client.id.clone();
            mgr.register(client).await;
            stopped_counts.push(count);
            stopped_ids.push(id);
        }

        // Force degraded and stopped status on the failing clients so the
        // scheduler uses the right intervals from the start.
        // We also set consecutive_errors high enough that record_error()
        // won't change their status (they're already at or past threshold).
        {
            let mut mm = mgr.mailboxes.lock().await;
            for id in &degraded_ids {
                let t = &mut mm.get_mut(id).unwrap().tracker;
                t.status = SyncStatus::Degraded;
                t.consecutive_errors = 1; // at degraded_threshold
            }
            for id in &stopped_ids {
                let t = &mut mm.get_mut(id).unwrap().tracker;
                t.status = SyncStatus::Stopped;
                t.consecutive_errors = 1000; // at stopped_threshold
            }
        }

        // Let the loop run for 60 simulated seconds.
        tokio::time::sleep(Duration::from_secs(60)).await;
        tokio::task::yield_now().await;

        let active_polls: Vec<u32> = active_counts
            .iter()
            .map(|c| c.load(Ordering::Relaxed))
            .collect();
        let degraded_polls: Vec<u32> = degraded_counts
            .iter()
            .map(|c| c.load(Ordering::Relaxed))
            .collect();
        let stopped_polls: Vec<u32> = stopped_counts
            .iter()
            .map(|c| c.load(Ordering::Relaxed))
            .collect();

        let avg_active = active_polls.iter().sum::<u32>() as f64 / active_polls.len() as f64;
        let avg_degraded = degraded_polls.iter().sum::<u32>() as f64 / degraded_polls.len() as f64;
        let avg_stopped = stopped_polls.iter().sum::<u32>() as f64 / stopped_polls.len() as f64;

        eprintln!("active polls:   {active_polls:?}  (avg {avg_active:.1})");
        eprintln!("degraded polls: {degraded_polls:?}  (avg {avg_degraded:.1})");
        eprintln!("stopped polls:  {stopped_polls:?}  (avg {avg_stopped:.1})");

        // Sanity: everyone got polled
        assert!(active_polls.iter().all(|&c| c > 0), "all active polled");
        assert!(degraded_polls.iter().all(|&c| c > 0), "all degraded polled");
        assert!(stopped_polls.iter().all(|&c| c > 0), "all stopped polled");

        // Within-group fairness: clients with the same status should not
        // deviate by more than 1 poll from each other.
        assert_within_group("active", &active_polls, 1);
        assert_within_group("degraded", &degraded_polls, 1);
        assert_within_group("stopped", &stopped_polls, 1);

        // Cross-group ratios should match the inverse of effective intervals.
        //   effective_interval = status_interval + between_polls_delay
        //   ratio(a/b) = effective_b / effective_a
        let expected_active_stopped = 2.2 / 1.2; // ≈ 1.83
        let expected_active_degraded = 1.7 / 1.2; // ≈ 1.42

        let ratio_active_stopped = avg_active / avg_stopped;
        let ratio_active_degraded = avg_active / avg_degraded;

        eprintln!(
            "ratio active/stopped:  {ratio_active_stopped:.2}  (expected {expected_active_stopped:.2})"
        );
        eprintln!(
            "ratio active/degraded: {ratio_active_degraded:.2}  (expected {expected_active_degraded:.2})"
        );

        let tolerance = 0.3;
        assert!(
            (ratio_active_stopped - expected_active_stopped).abs() < tolerance,
            "active/stopped ratio {ratio_active_stopped:.2} too far from expected {expected_active_stopped:.2}"
        );
        assert!(
            (ratio_active_degraded - expected_active_degraded).abs() < tolerance,
            "active/degraded ratio {ratio_active_degraded:.2} too far from expected {expected_active_degraded:.2}"
        );
    }
}
