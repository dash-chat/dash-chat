use crate::store::MailboxStore;
use crate::sync_tracker::MailboxSyncTracker;
use chrono::{DateTime, Utc};
use serde::{Serialize, Serializer};
use tokio::sync::watch;
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
pub struct LastError {
    pub at: DateTime<Utc>,
    pub message: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct MailboxConnectionState {
    pub status: SyncStatus,
    pub consecutive_errors: u32,
    #[serde(rename = "next_poll_in_ms", serialize_with = "ser_next_poll_in_ms")]
    pub next_poll: Instant,
    pub last_success_at: Option<DateTime<Utc>>,
    pub last_error: Option<LastError>,
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
        self.last_error = Some(LastError {
            at: Utc::now(),
            message: err,
        });
    }

    fn reschedule(&mut self, config: &MailboxesConfig) {
        self.next_poll = Instant::now() + self.status.interval(config) + config.between_polls_delay;
    }

    /// Treat this mailbox as healthy again and poll it immediately
    fn wakeup(&mut self) {
        self.status = SyncStatus::Active;
        self.consecutive_errors = 0;
        self.next_poll = Instant::now();
    }

    /// Become due immediately unless this mailbox has stopped retrying
    fn mark_due_now_unless_stopped(&mut self) {
        if self.status != SyncStatus::Stopped {
            self.next_poll = Instant::now();
        }
    }
}

/// Per-mailbox handle owning the client and its tracker. Held inside `Mailboxes`
/// as `Arc<TrackedMailbox<...>>` so cheap clones can be handed out to subscribers.
/// The client is held behind a `Mutex` so re-registering a mailbox can swap it
/// in place (e.g. mDNS re-resolution producing a new URL) without disturbing
/// the `connection_state` watcher's existing subscribers.
pub struct TrackedMailbox<Item: MailboxItem> {
    pub(crate) client: Mutex<Arc<dyn MailboxClient<Item>>>,
    pub(crate) connection_state: watch::Sender<MailboxConnectionState>,
}

impl<Item: MailboxItem> TrackedMailbox<Item> {
    fn new(client: Arc<dyn MailboxClient<Item>>) -> Self {
        let (connection_state, _) = watch::channel(MailboxConnectionState::new());
        Self {
            client: Mutex::new(client),
            connection_state,
        }
    }

    pub async fn client(&self) -> Arc<dyn MailboxClient<Item>> {
        self.client.lock().await.clone()
    }

    async fn replace_client(&self, client: Arc<dyn MailboxClient<Item>>) {
        *self.client.lock().await = client;
    }

    pub fn connection_state(&self) -> watch::Receiver<MailboxConnectionState> {
        self.connection_state.subscribe()
    }

    fn record_success(&self, config: &MailboxesConfig) {
        self.connection_state
            .send_modify(|t| t.record_success(config));
    }

    fn record_error(&self, config: &MailboxesConfig, err: String) {
        self.connection_state
            .send_modify(|t| t.record_error(config, err));
    }

    fn reschedule(&self, config: &MailboxesConfig) {
        self.connection_state.send_modify(|t| t.reschedule(config));
    }

    fn wakeup(&self) {
        self.connection_state.send_modify(|t| t.wakeup());
    }

    fn mark_due_now_unless_stopped(&self) {
        self.connection_state
            .send_modify(|t| t.mark_due_now_unless_stopped());
    }
}

#[derive(Clone)]
pub struct Mailboxes<Item, Store>
where
    Item: MailboxItem,
    Store: MailboxStore<Item>,
{
    mailboxes: Arc<Mutex<BTreeMap<MailboxId, Arc<TrackedMailbox<Item>>>>>,
    active_mailbox_ids_tx: watch::Sender<BTreeSet<MailboxId>>,
    topics: Arc<Mutex<HashMap<Item::Topic, mpsc::Sender<Item>>>>,
    store: Store,
    sync_tracker: Arc<MailboxSyncTracker<Item::Topic, Item::Author>>,
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
        sync_tracker: Arc<MailboxSyncTracker<Item::Topic, Item::Author>>,
        config: MailboxesConfig,
        trigger: mpsc::Sender<Option<MailboxId>>,
    ) -> Self {
        let (active_mailbox_ids_tx, _) = watch::channel(BTreeSet::new());
        Self {
            mailboxes: Arc::new(Mutex::new(Default::default())),
            active_mailbox_ids_tx,
            topics: Arc::new(Mutex::new(Default::default())),
            store,
            sync_tracker,
            config,
            trigger,
        }
    }

    /// Subscribe to the set of currently-registered mailbox ids.
    pub fn active_mailbox_ids(&self) -> watch::Receiver<BTreeSet<MailboxId>> {
        self.active_mailbox_ids_tx.subscribe()
    }

    /// Persistent, watch-based view over every mailbox we've ever recorded sync state for.
    pub fn sync_tracker(&self) -> &Arc<MailboxSyncTracker<Item::Topic, Item::Author>> {
        &self.sync_tracker
    }

    pub async fn is_tracked(&self, id: &MailboxId) -> bool {
        self.mailboxes.lock().await.contains_key(id)
    }

    pub async fn tracked_mailbox(&self, id: &MailboxId) -> Option<Arc<TrackedMailbox<Item>>> {
        self.mailboxes.lock().await.get(id).cloned()
    }

    pub async fn register(&self, mailbox: impl MailboxClient<Item>) {
        // TODO: check for existing mailbox with different ID but same "URL" (which is currently abstracted away and inaccessible here, darn)
        // TODO: make the ID come from the mailbox server itself, e.g. for mDNS discovery the ID is set by the mDNS service, but multiple services could point to the same actual mailbox state.
        let id = mailbox.id();
        let new_client: Arc<dyn MailboxClient<Item>> = Arc::new(mailbox);

        // Persist the mailbox's URL so it can be re-identified by URL after a
        // restart even when it is not currently registered (e.g. the cloud
        // mailbox whose id is otherwise only resolvable from its live server).
        if let Some(url) = new_client.url() {
            if let Err(err) = self.sync_tracker.record_url(&id, &url).await {
                tracing::error!(?err, mailbox = %id, "failed to record mailbox url");
            }
        }

        let mut mailboxes = self.mailboxes.lock().await;
        if let Some(tm) = mailboxes.get(&id).cloned() {
            drop(mailboxes);
            // Re-registering the same id (e.g. mDNS re-resolution producing a
            // new URL): swap the client in place and poll it right away, so a
            // backed-off mailbox recovers as soon as the new URL works. Keeping
            // the existing TrackedMailbox preserves its connection_state
            // watch::Sender so UI subscribers stay attached.
            tm.replace_client(new_client).await;
            tm.wakeup();
            self.trigger_sync();
        } else {
            mailboxes.insert(id.clone(), Arc::new(TrackedMailbox::new(new_client)));
            drop(mailboxes);
            self.publish_active_ids().await;
            self.trigger_sync();
        }
    }

    /// Returns `true` if a mailbox with the given id was removed.
    pub async fn unregister(&self, id: &MailboxId) -> bool {
        let mut mailboxes = self.mailboxes.lock().await;
        if mailboxes.remove(id).is_some() {
            drop(mailboxes);
            self.publish_active_ids().await;
            true
        } else {
            false
        }
    }

    pub async fn clear(&self) {
        self.mailboxes.lock().await.clear();
        self.publish_active_ids().await;
    }

    async fn publish_active_ids(&self) {
        let ids: BTreeSet<MailboxId> = self.mailboxes.lock().await.keys().cloned().collect();
        self.active_mailbox_ids_tx.send_replace(ids);
    }

    pub async fn subscribed_topics(&self) -> BTreeSet<Item::Topic> {
        self.topics.lock().await.keys().cloned().collect()
    }

    /// Returns the iroh EndpointIds of mailboxes currently tracking `topic`.
    pub async fn get_sources(&self, topic: &Item::Topic) -> anyhow::Result<Vec<iroh::EndpointId>> {
        if !self.topics.lock().await.contains_key(topic) {
            return Ok(Vec::new());
        }
        let ids: Vec<MailboxId> = self.mailboxes.lock().await.keys().cloned().collect();
        Ok(ids
            .into_iter()
            .filter_map(|id| mailbox_server::decode_mailbox_id(&id).ok())
            .collect())
    }

    pub fn trigger_sync(&self) {
        _ = self.trigger.try_send(None);
    }

    /// Immediately activate and sync a specific mailbox, resetting any backoff.
    pub fn wakeup(&self, id: MailboxId) {
        _ = self.trigger.try_send(Some(id));
    }

    /// Sync now instead of at the next scheduled poll
    pub async fn attempt_immediate_sync(&self) {
        for tracked_mailbox in self.mailboxes.lock().await.values() {
            tracked_mailbox.mark_due_now_unless_stopped();
        }
        self.trigger_sync();
    }

    /// Immediately activate and sync every registered mailbox, resetting any backoff.
    pub async fn wakeup_all(&self) {
        for tracked_mailbox in self.mailboxes.lock().await.values() {
            tracked_mailbox.wakeup();
        }
        self.trigger_sync();
    }

    pub async fn subscribe(
        &self,
        topic: Item::Topic,
    ) -> Result<Option<mpsc::Receiver<Item>>, anyhow::Error> {
        tracing::info!(topic = ?topic, "subscribing to topic");

        let mut tt = self.topics.lock().await;
        if tt.contains_key(&topic) {
            return Ok(None);
        }
        let (tx, rx) = mpsc::channel(100);
        tt.insert(topic, tx);
        Ok(Some(rx))
    }

    pub async fn unsubscribe(&self, topic: Item::Topic) -> Result<(), anyhow::Error> {
        tracing::info!(topic = ?topic, "unsubscribing from topic");
        self.topics.lock().await.remove(&topic);
        Ok(())
    }

    pub async fn spawn(
        store: Store,
        sync_tracker: Arc<MailboxSyncTracker<Item::Topic, Item::Author>>,
        config: MailboxesConfig,
    ) -> Result<Self, anyhow::Error> {
        let (trigger_tx, mut trigger_rx) = mpsc::channel::<Option<MailboxId>>(1);
        let manager = Self::new(store, sync_tracker, config, trigger_tx);
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
                                    Ok(Some(None)) => {} // general nudge, re-evaluate
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

        tracing::debug!("polling mailbox {id}");
        let client = tracked_mailbox.client().await;
        let result = self.sync_topics(topics.into_iter(), &client).await;

        match result {
            Ok(()) => tracked_mailbox.record_success(&self.config),
            Err(err) => {
                tracing::error!(?err, mailbox = %id, "mailbox sync error");
                tracked_mailbox.record_error(&self.config, format!("{err:?}"));
                let tracker = tracked_mailbox.connection_state();
                let tracker = tracker.borrow();
                tracing::info!(
                    mailbox = %id,
                    status = ?tracker.status,
                    errors = tracker.consecutive_errors,
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
        mailbox: &Arc<dyn MailboxClient<Item>>,
    ) -> anyhow::Result<()> {
        let mut request = BTreeMap::new();
        let mut sent_heights: BTreeMap<Item::Topic, BTreeMap<Item::Author, u64>> = BTreeMap::new();
        for topic in topics {
            let heights =
                BTreeMap::from_iter(self.store.get_log_heights(&topic).await?.into_iter());
            sent_heights.insert(topic, heights.clone());
            request.insert(topic, heights);
        }

        let FetchResponse(response) = mailbox.fetch(FetchRequest(request)).await?;

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
                tracing::warn!(topic = ?topic, "no sender for topic");
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
                    tracing::error!(author = ?author, topic = ?topic, lowest = ?lowest, "no log found");
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
        // ACID: a power cut here would lose these ops_to_publish.
        let publish_acks: Vec<(Item::Topic, Item::Author, u64)> = ops_to_publish
            .iter()
            .map(|op| (op.topic(), op.author(), op.seq_num()))
            .collect();

        mailbox.publish(ops_to_publish).await?;

        acks.extend(publish_acks);
        if let Err(err) = self.sync_tracker.record_synced(&mailbox.id(), &acks).await {
            tracing::error!(?err, mailbox = %&mailbox.id(), "failed to record sync watermarks");
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

    fn test_sync_tracker() -> Arc<MailboxSyncTracker<u8, char>> {
        Arc::new(MailboxSyncTracker::in_memory())
    }

    /// Create a Mailboxes instance without spawning the background loop
    fn test_mailboxes(config: MailboxesConfig) -> Mailboxes<Msg, DummyStore> {
        let (trigger_tx, _trigger_rx) = mpsc::channel(1);
        Mailboxes::new(DummyStore, test_sync_tracker(), config, trigger_tx)
    }

    async fn spawn_test_mailboxes(config: MailboxesConfig) -> Mailboxes<Msg, DummyStore> {
        Mailboxes::<Msg, DummyStore>::spawn(DummyStore, test_sync_tracker(), config)
            .await
            .unwrap()
    }

    // -- MailboxTracker unit tests --

    #[tokio::test(start_paused = true)]
    async fn tracker_starts_active() {
        let tracker = MailboxConnectionState::new();
        assert_eq!(tracker.status, SyncStatus::Active);
        assert_eq!(tracker.consecutive_errors, 0);
    }

    #[tokio::test(start_paused = true)]
    async fn tracker_success_resets_to_active() {
        let config = test_config();
        let mut tracker = MailboxConnectionState::new();

        // Accumulate some errors first
        tracker.record_error(&config, "x".into());
        tracker.record_error(&config, "x".into());
        assert_eq!(tracker.status, SyncStatus::Degraded);

        // Success resets everything
        tracker.record_success(&config);
        assert_eq!(tracker.status, SyncStatus::Active);
        assert_eq!(tracker.consecutive_errors, 0);
        assert!(tracker.last_success_at.is_some());
        assert!(tracker.last_error.is_none());
    }

    #[tokio::test(start_paused = true)]
    async fn tracker_error_transitions() {
        let config = test_config();
        let mut tracker = MailboxConnectionState::new();

        // 1 error: still Active
        tracker.record_error(&config, "x".into());
        assert_eq!(tracker.status, SyncStatus::Active);
        assert_eq!(tracker.consecutive_errors, 1);

        // 2 errors: Degraded
        tracker.record_error(&config, "x".into());
        assert_eq!(tracker.status, SyncStatus::Degraded);
        assert_eq!(tracker.consecutive_errors, 2);

        // 3 errors: Stopped
        tracker.record_error(&config, "x".into());
        assert_eq!(tracker.status, SyncStatus::Stopped);
        assert_eq!(tracker.consecutive_errors, 3);

        // More errors: stays Stopped
        tracker.record_error(&config, "x".into());
        assert_eq!(tracker.status, SyncStatus::Stopped);
        assert_eq!(tracker.consecutive_errors, 4);
        assert!(tracker.last_error.is_some());
    }

    #[tokio::test(start_paused = true)]
    async fn tracker_next_poll_after_success() {
        let config = test_config();
        let mut tracker = MailboxConnectionState::new();

        let before = Instant::now();
        tracker.record_success(&config);
        let expected = before + config.active_interval + config.between_polls_delay;

        assert_eq!(tracker.next_poll, expected);
    }

    #[tokio::test(start_paused = true)]
    async fn tracker_next_poll_after_errors() {
        let config = test_config();
        let mut tracker = MailboxConnectionState::new();
        let delay = config.between_polls_delay;

        // First error: still Active interval
        tracker.record_error(&config, "x".into());
        let expected = Instant::now() + config.active_interval + delay;
        assert_eq!(tracker.next_poll, expected);

        tokio::time::advance(Duration::from_secs(1)).await;

        // Second error: Degraded interval
        tracker.record_error(&config, "x".into());
        let expected = Instant::now() + config.degraded_interval + delay;
        assert_eq!(tracker.next_poll, expected);

        tokio::time::advance(Duration::from_secs(1)).await;

        // Third error: Stopped interval
        tracker.record_error(&config, "x".into());
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
            let mm = mgr.mailboxes.lock().await;
            mm.get(&id1).unwrap().record_success(&config);
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
            let mm = mgr.mailboxes.lock().await;
            mm.get(&id).unwrap().record_success(&config);
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
            let mm = mgr.mailboxes.lock().await;
            let t = mm.get(&id).unwrap();
            t.record_error(&config, "x".into()); // 1 error: Active
            t.record_error(&config, "x".into()); // 2 errors: Degraded
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

        // Schedule into the future
        {
            let mm = mgr.mailboxes.lock().await;
            mm.get(&id).unwrap().record_success(&config);
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
            let mm = mgr.mailboxes.lock().await;
            mm.get(&id_a).unwrap().record_success(&config);
            let b = mm.get(&id_b).unwrap();
            b.record_error(&config, "x".into());
            b.record_error(&config, "x".into());
            let c = mm.get(&id_c).unwrap();
            c.record_error(&config, "x".into());
            c.record_error(&config, "x".into());
            c.record_error(&config, "x".into());
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

    // -- active_mailbox_ids tests --

    #[tokio::test(start_paused = true)]
    async fn active_mailbox_ids_updates_on_register_and_clear() {
        let mgr = test_mailboxes(test_config());
        let mut rx = mgr.active_mailbox_ids();
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

    #[tokio::test(start_paused = true)]
    async fn active_mailbox_ids_subscribed_after_register_sees_registered() {
        let mgr = test_mailboxes(test_config());

        let mb = MemMailbox::<Msg>::new();
        let id = mb.client().id();
        mgr.register(mb.client()).await;

        let rx = mgr.active_mailbox_ids();
        assert!(rx.borrow().contains(&id));
    }

    /// Re-registering the same mailbox id (e.g. mDNS re-resolution producing a
    /// fresh URL) swaps the underlying client in place but must preserve the
    /// existing `connection_state` watch::Sender — otherwise UI subscribers get
    /// a closed channel and stop observing later state transitions.
    #[tokio::test(start_paused = true)]
    async fn re_register_preserves_connection_state_subscribers() {
        let config = test_config();
        let mgr = test_mailboxes(config.clone());

        let mb = MemMailbox::<Msg>::new();
        let client = mb.client();
        let id = client.id();
        mgr.register(client).await;

        let mut rx = {
            let mm = mgr.mailboxes.lock().await;
            mm.get(&id).unwrap().connection_state()
        };

        mgr.register(mb.client()).await;

        // The watcher is still alive: a later state transition reaches the
        // subscriber instead of erroring out with a closed channel.
        {
            let mm = mgr.mailboxes.lock().await;
            mm.get(&id).unwrap().record_success(&config);
        }
        rx.changed().await.expect("subscriber channel closed");
        assert_eq!(rx.borrow().status, SyncStatus::Active);
    }

    #[tokio::test(start_paused = true)]
    async fn re_register_replaces_client_and_wakes_stopped_mailbox() {
        let config = MailboxesConfig {
            active_interval: Duration::from_secs(100),
            stopped_interval: Duration::from_secs(600),
            between_polls_delay: Duration::from_millis(0),
            ..test_config()
        };

        let mgr = spawn_test_mailboxes(config.clone()).await;
        let _rx = mgr.subscribe(0u8).await.unwrap();

        // Register an initial client and wait for its first poll.
        let (client_a, count_a) = TrackingClient::new(false);
        let id = client_a.id.clone();
        mgr.register(client_a).await;
        for _ in 0..10 {
            tokio::task::yield_now().await;
        }
        let polls_a_after_register = count_a.load(Ordering::Relaxed);
        assert!(polls_a_after_register >= 1);

        // Force the mailbox into Stopped so without an explicit wake it would not
        // poll again for ~600s.
        {
            let mm = mgr.mailboxes.lock().await;
            let t = mm.get(&id).unwrap();
            t.connection_state.send_modify(|s| {
                s.status = SyncStatus::Stopped;
                s.consecutive_errors = 100;
                s.next_poll = Instant::now() + Duration::from_secs(600);
            });
        }

        // Re-register with a different client carrying the same id — the loop
        // should poll the *new* client immediately thanks to being marked due.
        let mut client_b = TrackingClient::new(false).0;
        client_b.id = id.clone();
        let count_b = client_b.poll_count.clone();
        let polls_a_before_swap = count_a.load(Ordering::Relaxed);
        mgr.register(client_b).await;
        for _ in 0..10 {
            tokio::task::yield_now().await;
        }

        assert!(count_b.load(Ordering::Relaxed) >= 1);
        assert_eq!(count_a.load(Ordering::Relaxed), polls_a_before_swap);
        let mm = mgr.mailboxes.lock().await;
        assert_eq!(
            mm.get(&id).unwrap().connection_state().borrow().status,
            SyncStatus::Active
        );
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

        // Put mailbox in Stopped state
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
        let tracker = mm.get(&id).unwrap().connection_state();
        let tracker = tracker.borrow();
        assert_eq!(tracker.status, SyncStatus::Active);
        assert_eq!(tracker.consecutive_errors, 0);
    }

    #[tokio::test(start_paused = true)]
    async fn attempt_immediate_sync_leaves_stopped_on_its_schedule() {
        let config = test_config();
        let mgr = test_mailboxes(config.clone());

        let degraded = MemMailbox::<Msg>::new();
        let degraded_id = degraded.client().id();
        mgr.register(degraded.client()).await;

        let stopped = MemMailbox::<Msg>::new();
        let stopped_id = stopped.client().id();
        mgr.register(stopped.client()).await;

        {
            let mm = mgr.mailboxes.lock().await;
            let d = mm.get(&degraded_id).unwrap();
            d.record_error(&config, "x".into());
            d.record_error(&config, "x".into());
            assert_eq!(d.connection_state().borrow().status, SyncStatus::Degraded);

            let s = mm.get(&stopped_id).unwrap();
            s.record_error(&config, "x".into());
            s.record_error(&config, "x".into());
            s.record_error(&config, "x".into());
            assert_eq!(s.connection_state().borrow().status, SyncStatus::Stopped);
        }

        mgr.attempt_immediate_sync().await;

        let mm = mgr.mailboxes.lock().await;
        let now = Instant::now();
        assert_eq!(
            mm.get(&degraded_id)
                .unwrap()
                .connection_state()
                .borrow()
                .next_poll,
            now
        );
        assert!(
            mm.get(&stopped_id)
                .unwrap()
                .connection_state()
                .borrow()
                .next_poll
                > now
        );
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

        // Let the initial poll (triggered by register) complete
        for _ in 0..10 {
            tokio::task::yield_now().await;
        }
        assert_eq!(poll_count.load(Ordering::Relaxed), 1);

        // Force the mailbox into Stopped with a far-future next_poll
        {
            let mm = mgr.mailboxes.lock().await;
            let t = mm.get(&id).unwrap();
            t.connection_state.send_modify(|s| {
                s.status = SyncStatus::Stopped;
                s.consecutive_errors = 100;
                s.next_poll = Instant::now() + Duration::from_secs(600);
            });
        }

        // Without an immediate-sync request, no poll should happen (time is paused)
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

        let mgr = spawn_test_mailboxes(config).await;

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
            let mm = mgr.mailboxes.lock().await;
            for id in &degraded_ids {
                let t = mm.get(id).unwrap();
                t.connection_state.send_modify(|s| {
                    s.status = SyncStatus::Degraded;
                    s.consecutive_errors = 1; // at degraded_threshold
                });
            }
            for id in &stopped_ids {
                let t = mm.get(id).unwrap();
                t.connection_state.send_modify(|s| {
                    s.status = SyncStatus::Stopped;
                    s.consecutive_errors = 1000; // at stopped_threshold
                });
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
