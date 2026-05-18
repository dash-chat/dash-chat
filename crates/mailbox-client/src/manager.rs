use crate::mailbox_tracker_store::MailboxTrackerStore;
use crate::store::MailboxStore;
use chrono::{DateTime, Utc};
use serde::{Serialize, Serializer};
use tokio::sync::watch;
use tokio::time::Instant;

#[cfg(feature = "named-id")]
use named_id::Rename;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
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

#[derive(Clone, Debug, Serialize)]
pub struct MailboxConnectionState {
    pub status: SyncStatus,
    pub consecutive_errors: u32,
    #[serde(rename = "next_poll_in_ms", serialize_with = "ser_next_poll_in_ms")]
    pub next_poll: Instant,
    pub last_success_at: Option<DateTime<Utc>>,
    pub last_error_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
}

fn ser_next_poll_in_ms<S: Serializer>(next: &Instant, s: S) -> Result<S::Ok, S::Error> {
    let now = Instant::now();
    let ms: i64 = if *next >= now {
        next.duration_since(now).as_millis().min(i64::MAX as u128) as i64
    } else {
        -(now.duration_since(*next).as_millis().min(i64::MAX as u128) as i64)
    };
    s.serialize_i64(ms)
}

impl MailboxConnectionState {
    fn new() -> Self {
        Self {
            status: SyncStatus::Active,
            consecutive_errors: 0,
            next_poll: Instant::now(),
            last_success_at: None,
            last_error_at: None,
            last_error: None,
        }
    }

    fn record_success(&mut self, config: &MailboxesConfig) {
        self.consecutive_errors = 0;
        self.status = SyncStatus::Active;
        self.next_poll = Instant::now() + config.active_interval + config.between_polls_delay;
        self.last_success_at = Some(Utc::now());
        self.last_error = None;
    }

    fn record_error(&mut self, config: &MailboxesConfig, err: String) {
        self.consecutive_errors += 1;
        self.status = if self.consecutive_errors >= config.stopped_threshold {
            SyncStatus::Stopped
        } else if self.consecutive_errors >= config.degraded_threshold {
            SyncStatus::Degraded
        } else {
            self.status
        };
        self.next_poll = Instant::now() + self.status.interval(config) + config.between_polls_delay;
        self.last_error_at = Some(Utc::now());
        self.last_error = Some(err);
    }

    fn reschedule(&mut self, config: &MailboxesConfig) {
        self.next_poll = Instant::now() + self.status.interval(config) + config.between_polls_delay;
    }

    fn wakeup(&mut self) {
        self.status = SyncStatus::Active;
        self.consecutive_errors = 0;
        self.next_poll = Instant::now();
    }
}

pub type MailboxSyncState<T, A> = BTreeMap<(T, A), u64>;

/// Per-mailbox handle owning the client, its connection state, and its sync watermarks.
/// Held inside `Mailboxes` as `Arc<TrackedMailbox<...>>` so cheap clones can be handed out
/// to subscribers.
pub struct TrackedMailbox<Item: MailboxItem> {
    id: MailboxId,
    client: Arc<dyn MailboxClient<Item>>,
    connection_state_tx: watch::Sender<MailboxConnectionState>,
    sync_state_tx: watch::Sender<MailboxSyncState<Item::Topic, Item::Author>>,
    tracker_store: Arc<MailboxTrackerStore>,
}

impl<Item: MailboxItem> TrackedMailbox<Item> {
    async fn init(
        id: MailboxId,
        client: Arc<dyn MailboxClient<Item>>,
        tracker_store: Arc<MailboxTrackerStore>,
    ) -> Self {
        let initial_sync = tracker_store
            .get_all_for_mailbox::<Item::Topic, Item::Author>(&id)
            .await
            .unwrap_or_else(|err| {
                tracing::error!(?err, mailbox = %id, "failed to load initial sync state");
                BTreeMap::new()
            });
        let (connection_state_tx, _) = watch::channel(MailboxConnectionState::new());
        let (sync_state_tx, _) = watch::channel(initial_sync);
        Self {
            id,
            client,
            connection_state_tx,
            sync_state_tx,
            tracker_store,
        }
    }

    pub fn id(&self) -> &MailboxId {
        &self.id
    }

    pub fn connection_state(&self) -> watch::Receiver<MailboxConnectionState> {
        self.connection_state_tx.subscribe()
    }

    pub fn sync_state(&self) -> watch::Receiver<MailboxSyncState<Item::Topic, Item::Author>> {
        self.sync_state_tx.subscribe()
    }

    fn record_success(&self, config: &MailboxesConfig) {
        self.connection_state_tx
            .send_modify(|s| s.record_success(config));
    }

    fn record_error(&self, config: &MailboxesConfig, err: String) {
        self.connection_state_tx
            .send_modify(|s| s.record_error(config, err));
    }

    fn reschedule(&self, config: &MailboxesConfig) {
        self.connection_state_tx
            .send_modify(|s| s.reschedule(config));
    }

    fn wakeup(&self) {
        self.connection_state_tx.send_modify(|s| s.wakeup());
    }

    async fn record_synced(
        &self,
        topic: Item::Topic,
        author: Item::Author,
        seq: u64,
    ) -> anyhow::Result<()> {
        self.tracker_store
            .record_synced(&self.id, &topic, &author, seq)
            .await?;
        self.sync_state_tx.send_modify(|m| {
            let entry = m.entry((topic, author)).or_insert(0);
            if seq > *entry {
                *entry = seq;
            }
        });
        Ok(())
    }
}

#[derive(Clone)]
pub struct Mailboxes<Item, Store>
where
    Item: MailboxItem,
    Store: MailboxStore<Item>,
{
    mailboxes: Arc<Mutex<BTreeMap<MailboxId, Arc<TrackedMailbox<Item>>>>>,
    mailbox_ids_tx: watch::Sender<BTreeSet<MailboxId>>,
    topics: Arc<Mutex<HashMap<Item::Topic, mpsc::Sender<Item>>>>,
    store: Store,
    mailbox_tracker_store: Arc<MailboxTrackerStore>,
    config: MailboxesConfig,
    trigger: mpsc::Sender<Option<MailboxId>>,
}

impl<Item, Store> Mailboxes<Item, Store>
where
    Item: MailboxItem,
    Store: MailboxStore<Item>,
    Item::Topic: OptionalItemTraits,
{
    fn new(
        store: Store,
        mailbox_tracker_store: Arc<MailboxTrackerStore>,
        config: MailboxesConfig,
        trigger: mpsc::Sender<Option<MailboxId>>,
    ) -> Self {
        let (mailbox_ids_tx, _) = watch::channel(BTreeSet::new());
        Self {
            mailboxes: Arc::new(Mutex::new(Default::default())),
            mailbox_ids_tx,
            topics: Arc::new(Mutex::new(Default::default())),
            store,
            mailbox_tracker_store,
            config,
            trigger,
        }
    }

    pub fn mailbox_ids(&self) -> watch::Receiver<BTreeSet<MailboxId>> {
        self.mailbox_ids_tx.subscribe()
    }

    pub async fn tracked_mailbox(&self, id: &MailboxId) -> Option<Arc<TrackedMailbox<Item>>> {
        self.mailboxes.lock().await.get(id).cloned()
    }

    pub async fn register(&self, mailbox: impl MailboxClient<Item>) {
        // TODO: check for existing mailbox with different ID but same "URL" (which is currently abstracted away and inaccessible here, darn)
        // TODO: make the ID come from the mailbox server itself, e.g. for mDNS discovery the ID is set by the mDNS service, but multiple services could point to the same actual mailbox state.
        let id = mailbox.id();
        let tracked_mailbox = Arc::new(
            TrackedMailbox::init(
                id.clone(),
                Arc::new(mailbox),
                self.mailbox_tracker_store.clone(),
            )
            .await,
        );
        let existing = self
            .mailboxes
            .lock()
            .await
            .insert(id.clone(), tracked_mailbox);
        if existing.is_some() {
            // TODO: potentially track multiple clients for a single mailbox ID, e.g. multiple mDNS discovered addresses for the same node
            // TODO: at least, make sure the URL being replaced is "better" than the previous one, i.e. ipv4 instead of ipv6
            tracing::warn!("overwriting existing mailbox for {id}");
        }
        self.publish_ids().await;
        self.trigger_sync();
    }

    pub async fn clear(&self) {
        self.mailboxes.lock().await.clear();
        self.publish_ids().await;
    }

    async fn publish_ids(&self) {
        let ids: BTreeSet<MailboxId> = self.mailboxes.lock().await.keys().cloned().collect();
        let _ = self.mailbox_ids_tx.send(ids);
    }

    pub async fn subscribed_topics(&self) -> BTreeSet<Item::Topic> {
        self.topics.lock().await.keys().cloned().collect()
    }

    pub fn trigger_sync(&self) {
        _ = self.trigger.try_send(None);
    }

    /// Immediately activate and sync a specific mailbox, resetting any backoff.
    pub fn wakeup(&self, id: MailboxId) {
        _ = self.trigger.try_send(Some(id));
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

    pub async fn spawn(
        store: Store,
        mailbox_tracker_store: Arc<MailboxTrackerStore>,
        config: MailboxesConfig,
    ) -> Result<Self, anyhow::Error> {
        let (trigger_tx, mut trigger_rx) = mpsc::channel::<Option<MailboxId>>(1);
        let manager = Self::new(store, mailbox_tracker_store, config, trigger_tx);
        let r = manager.clone();
        tokio::spawn(
            async move {
                loop {
                    let next = manager.find_next_due().await;

                    match next {
                        None => {
                            // No mailboxes registered, wait for a trigger
                            match trigger_rx.recv().await {
                                Some(msg) => {
                                    if let Some(id) = msg {
                                        manager.wakeup_mailbox(&id).await;
                                    }
                                    continue;
                                }
                                None => break,
                            }
                        }
                        Some((id, wait)) => {
                            if !wait.is_zero() {
                                // Sleep until the next mailbox is due, or a trigger wakes us
                                match tokio::time::timeout(wait, trigger_rx.recv()).await {
                                    Ok(None) => break, // channel closed
                                    Ok(Some(Some(triggered_id))) => {
                                        manager.wakeup_mailbox(&triggered_id).await;
                                    }
                                    Ok(Some(None)) => {} // general wakeup, re-evaluate
                                    Err(_) => {}         // timeout elapsed
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
        let (id, tracked) = mm
            .iter()
            .min_by_key(|(_, t)| t.connection_state().borrow().next_poll)
            .unwrap();

        let next = tracked.connection_state().borrow().next_poll;
        let wait = if next <= now {
            Duration::ZERO
        } else {
            next - now
        };

        Some((id.clone(), wait))
    }

    async fn wakeup_mailbox(&self, id: &MailboxId) {
        let mm = self.mailboxes.lock().await;
        if let Some(tracked_mailbox) = mm.get(id) {
            tracked_mailbox.wakeup();
        }
    }

    async fn poll_mailbox(&self, id: &MailboxId) {
        let tracked_mailbox = {
            let mm = self.mailboxes.lock().await;
            match mm.get(id) {
                Some(t) => t.clone(),
                None => return,
            }
        };

        let topics = self.subscribed_topics().await;
        if topics.is_empty() {
            tracing::trace!("no topics subscribed, skipping poll for {id}");
            tracked_mailbox.reschedule(&self.config);
            return;
        }

        tracing::info!("polling mailbox {id}");
        let result = self.sync_topics(topics.into_iter(), &tracked_mailbox).await;

        match result {
            Ok(()) => tracked_mailbox.record_success(&self.config),
            Err(err) => {
                tracing::error!(?err, mailbox = %id, "mailbox sync error");
                tracked_mailbox.record_error(&self.config, format!("{err:?}"));
                let state = tracked_mailbox.connection_state();
                let state = state.borrow();
                tracing::info!(
                    mailbox = %id,
                    status = ?state.status,
                    errors = state.consecutive_errors,
                    "mailbox status updated"
                );
            }
        }
    }

    /// Immediately sync the given topics with the given mailbox:
    /// - Ensure all items held by the mailbox are fetched
    /// - Publish any items that the mailbox is missing to the mailbox
    async fn sync_topics(
        &self,
        topics: impl Iterator<Item = Item::Topic>,
        tracked_mailbox: &TrackedMailbox<Item>,
    ) -> anyhow::Result<()> {
        let mut request = BTreeMap::new();
        let mut sent_heights: BTreeMap<Item::Topic, BTreeMap<Item::Author, u64>> = BTreeMap::new();
        for topic in topics {
            let heights =
                BTreeMap::from_iter(self.store.get_log_heights(&topic).await?.into_iter());
            sent_heights.insert(topic, heights.clone());
            request.insert(topic, heights);
        }

        let FetchResponse(response) = tracked_mailbox.client.fetch(FetchRequest(request)).await?;

        let mut ops_to_publish: Vec<Item> = vec![];
        let mut acks: Vec<(Item::Topic, Item::Author, u64)> = vec![];

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

            // Sync watermark inference for authors we sent heights for:
            // if the server returned no `missing` entries for an author, it has the log
            // contiguously up to at least the height we sent.
            if let Some(heights) = sent_heights.get(&topic) {
                for (author, height) in heights {
                    if !missing.contains_key(author) {
                        acks.push((topic, *author, *height));
                    }
                }
            }

            // Each received item is one the mailbox already has.
            for item in &items {
                acks.push((item.topic(), item.author(), item.seq_num()));
            }

            let Some(sender) = self.topics.lock().await.get(&topic).cloned() else {
                #[cfg(feature = "named-id")]
                tracing::warn!(topic = ?topic.renamed(), "no sender for topic");
                continue;
            };

            for item in items {
                sender.send(item).await?;
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
                    tracing::error!(author = ?author.renamed(), topic = ?topic.renamed(), lowest = ?lowest, "no log found");
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

        // For ops we successfully publish, the mailbox now has at least their seq_num.
        let publish_acks: Vec<(Item::Topic, Item::Author, u64)> = ops_to_publish
            .iter()
            .map(|op| (op.topic(), op.author(), op.seq_num()))
            .collect();

        tracked_mailbox.client.publish(ops_to_publish).await?;

        for (t, a, s) in acks.into_iter().chain(publish_acks) {
            if let Err(err) = tracked_mailbox.record_synced(t, a, s).await {
                tracing::error!(?err, mailbox = %tracked_mailbox.id, "failed to record sync watermark");
            }
        }

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

    fn test_tracker_store() -> Arc<MailboxTrackerStore> {
        Arc::new(MailboxTrackerStore::in_memory())
    }

    /// Create a Mailboxes instance without spawning the background loop
    fn test_mailboxes(config: MailboxesConfig) -> Mailboxes<Msg, DummyStore> {
        let (trigger_tx, _trigger_rx) = mpsc::channel(1);
        Mailboxes::new(DummyStore, test_tracker_store(), config, trigger_tx)
    }

    async fn spawn_test_mailboxes(config: MailboxesConfig) -> Mailboxes<Msg, DummyStore> {
        Mailboxes::<Msg, DummyStore>::spawn(DummyStore, test_tracker_store(), config)
            .await
            .unwrap()
    }

    // -- MailboxConnectionState unit tests --

    #[tokio::test(start_paused = true)]
    async fn state_starts_active() {
        let state = MailboxConnectionState::new();
        assert_eq!(state.status, SyncStatus::Active);
        assert_eq!(state.consecutive_errors, 0);
    }

    #[tokio::test(start_paused = true)]
    async fn state_success_resets_to_active() {
        let config = test_config();
        let mut state = MailboxConnectionState::new();

        state.record_error(&config, "x".into());
        state.record_error(&config, "x".into());
        assert_eq!(state.status, SyncStatus::Degraded);

        state.record_success(&config);
        assert_eq!(state.status, SyncStatus::Active);
        assert_eq!(state.consecutive_errors, 0);
        assert!(state.last_success_at.is_some());
        assert!(state.last_error.is_none());
    }

    #[tokio::test(start_paused = true)]
    async fn state_error_transitions() {
        let config = test_config();
        let mut state = MailboxConnectionState::new();

        state.record_error(&config, "x".into());
        assert_eq!(state.status, SyncStatus::Active);
        assert_eq!(state.consecutive_errors, 1);

        state.record_error(&config, "x".into());
        assert_eq!(state.status, SyncStatus::Degraded);
        assert_eq!(state.consecutive_errors, 2);

        state.record_error(&config, "x".into());
        assert_eq!(state.status, SyncStatus::Stopped);
        assert_eq!(state.consecutive_errors, 3);

        state.record_error(&config, "x".into());
        assert_eq!(state.status, SyncStatus::Stopped);
        assert_eq!(state.consecutive_errors, 4);
        assert!(state.last_error_at.is_some());
        assert!(state.last_error.is_some());
    }

    #[tokio::test(start_paused = true)]
    async fn state_next_poll_after_success() {
        let config = test_config();
        let mut state = MailboxConnectionState::new();

        let before = Instant::now();
        state.record_success(&config);
        let expected = before + config.active_interval + config.between_polls_delay;

        assert_eq!(state.next_poll, expected);
    }

    #[tokio::test(start_paused = true)]
    async fn state_next_poll_after_errors() {
        let config = test_config();
        let mut state = MailboxConnectionState::new();
        let delay = config.between_polls_delay;

        state.record_error(&config, "x".into());
        let expected = Instant::now() + config.active_interval + delay;
        assert_eq!(state.next_poll, expected);

        tokio::time::advance(Duration::from_secs(1)).await;

        state.record_error(&config, "x".into());
        let expected = Instant::now() + config.degraded_interval + delay;
        assert_eq!(state.next_poll, expected);

        tokio::time::advance(Duration::from_secs(1)).await;

        state.record_error(&config, "x".into());
        let expected = Instant::now() + config.stopped_interval + delay;
        assert_eq!(state.next_poll, expected);
    }

    // -- find_next_due tests --

    #[tokio::test]
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

        let (found_id, wait) = mgr.find_next_due().await.unwrap();
        assert_eq!(found_id, id);
        assert_eq!(wait, Duration::ZERO);
    }

    #[tokio::test(start_paused = true)]
    async fn find_next_due_picks_earliest() {
        let config = test_config();
        let mgr = test_mailboxes(config.clone());

        let mb1 = MemMailbox::<Msg>::new();
        let c1 = mb1.client();
        let id1 = c1.id();
        mgr.register(c1).await;

        {
            let mm = mgr.mailboxes.lock().await;
            mm.get(&id1).unwrap().record_success(&config);
        }

        tokio::time::advance(Duration::from_secs(2)).await;

        let mb2 = MemMailbox::<Msg>::new();
        let c2 = mb2.client();
        let id2 = c2.id();
        mgr.register(c2).await;

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

        {
            let mm = mgr.mailboxes.lock().await;
            mm.get(&id).unwrap().record_success(&config);
        }

        let (found_id, wait) = mgr.find_next_due().await.unwrap();
        assert_eq!(found_id, id);
        assert_eq!(wait, config.active_interval + delay);

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

        {
            let mm = mgr.mailboxes.lock().await;
            let t = mm.get(&id).unwrap();
            t.record_error(&config, "x".into());
            t.record_error(&config, "x".into());
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

        {
            let mm = mgr.mailboxes.lock().await;
            let t = mm.get(&id).unwrap();
            t.record_error(&config, "x".into());
            t.record_error(&config, "x".into());
            t.record_error(&config, "x".into());
            assert_eq!(t.connection_state().borrow().status, SyncStatus::Stopped);
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

        {
            let mm = mgr.mailboxes.lock().await;
            mm.get(&id).unwrap().record_success(&config);
        }

        tokio::time::advance(
            config.active_interval + config.between_polls_delay + Duration::from_secs(1),
        )
        .await;

        let (_found_id, wait) = mgr.find_next_due().await.unwrap();
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

    // -- mailbox_ids tests --

    #[tokio::test(start_paused = true)]
    async fn mailbox_ids_updates_on_register_and_clear() {
        let mgr = test_mailboxes(test_config());
        let mut rx = mgr.mailbox_ids();
        assert!(rx.borrow().is_empty());

        let mb = MemMailbox::<Msg>::new();
        let id = mb.client().id();
        mgr.register(mb.client()).await;

        rx.changed().await.unwrap();
        assert!(rx.borrow().contains(&id));

        mgr.clear().await;
        rx.changed().await.unwrap();
        assert!(rx.borrow().is_empty());
    }

    // -- wakeup tests --

    #[tokio::test(start_paused = true)]
    async fn wakeup_mailbox_resets_status_and_schedule() {
        let config = test_config();
        let mgr = test_mailboxes(config.clone());

        let mb = MemMailbox::<Msg>::new();
        let client = mb.client();
        let id = client.id();
        mgr.register(client).await;

        {
            let mm = mgr.mailboxes.lock().await;
            let t = mm.get(&id).unwrap();
            t.record_error(&config, "x".into());
            t.record_error(&config, "x".into());
            t.record_error(&config, "x".into());
            assert_eq!(t.connection_state().borrow().status, SyncStatus::Stopped);
        }

        let (_, wait) = mgr.find_next_due().await.unwrap();
        assert!(wait > Duration::ZERO);

        mgr.wakeup_mailbox(&id).await;

        let (found_id, wait) = mgr.find_next_due().await.unwrap();
        assert_eq!(found_id, id);
        assert_eq!(wait, Duration::ZERO);
        let mm = mgr.mailboxes.lock().await;
        let state = mm.get(&id).unwrap().connection_state();
        let state = state.borrow();
        assert_eq!(state.status, SyncStatus::Active);
        assert_eq!(state.consecutive_errors, 0);
    }

    #[tokio::test(start_paused = true)]
    async fn wakeup_polls_stopped_mailbox_immediately() {
        let config = MailboxesConfig {
            active_interval: Duration::from_secs(100),
            stopped_interval: Duration::from_secs(600),
            between_polls_delay: Duration::from_millis(0),
            ..test_config()
        };

        let mgr = spawn_test_mailboxes(config.clone()).await;

        let _rx = mgr.subscribe(0u8).await.unwrap();

        let (client, poll_count) = TrackingClient::new(false);
        let id = client.id.clone();
        mgr.register(client).await;

        for _ in 0..10 {
            tokio::task::yield_now().await;
        }
        assert_eq!(poll_count.load(Ordering::Relaxed), 1);

        {
            let mm = mgr.mailboxes.lock().await;
            let t = mm.get(&id).unwrap();
            t.connection_state_tx.send_modify(|s| {
                s.status = SyncStatus::Stopped;
                s.consecutive_errors = 100;
                s.next_poll = Instant::now() + Duration::from_secs(600);
            });
        }

        for _ in 0..5 {
            tokio::task::yield_now().await;
        }
        assert_eq!(poll_count.load(Ordering::Relaxed), 1);

        mgr.wakeup(id.clone());

        for _ in 0..10 {
            tokio::task::yield_now().await;
        }

        assert_eq!(poll_count.load(Ordering::Relaxed), 2);
        let mm = mgr.mailboxes.lock().await;
        assert_eq!(
            mm.get(&id).unwrap().connection_state().borrow().status,
            SyncStatus::Active
        );
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
        let config = MailboxesConfig {
            active_interval: Duration::from_secs(1),
            degraded_interval: Duration::from_millis(1500),
            stopped_interval: Duration::from_secs(2),
            between_polls_delay: Duration::from_millis(200),
            degraded_threshold: 1,
            stopped_threshold: 1000,
        };

        let mgr = spawn_test_mailboxes(config).await;

        let _rx = mgr.subscribe(0u8).await.unwrap();

        let mut active_counts = vec![];
        let mut degraded_counts = vec![];
        let mut stopped_counts = vec![];

        for _ in 0..5 {
            let (client, count) = TrackingClient::new(false);
            mgr.register(client).await;
            active_counts.push(count);
        }

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

        {
            let mm = mgr.mailboxes.lock().await;
            for id in &degraded_ids {
                let t = mm.get(id).unwrap();
                t.connection_state_tx.send_modify(|s| {
                    s.status = SyncStatus::Degraded;
                    s.consecutive_errors = 1;
                });
            }
            for id in &stopped_ids {
                let t = mm.get(id).unwrap();
                t.connection_state_tx.send_modify(|s| {
                    s.status = SyncStatus::Stopped;
                    s.consecutive_errors = 1000;
                });
            }
        }

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

        assert!(active_polls.iter().all(|&c| c > 0), "all active polled");
        assert!(degraded_polls.iter().all(|&c| c > 0), "all degraded polled");
        assert!(stopped_polls.iter().all(|&c| c > 0), "all stopped polled");

        assert_within_group("active", &active_polls, 1);
        assert_within_group("degraded", &degraded_polls, 1);
        assert_within_group("stopped", &stopped_polls, 1);

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
