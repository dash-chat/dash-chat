use std::fmt::Display;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use mailbox_client::operation_delivery::OperationDelivery;
use mailbox_client::store::MailboxStore;
use mailbox_client::sync_scheduler::{PendingRequest, RequestOutcome};
use mailbox_client::{
    MailboxClient, MailboxConnectionHealth, MailboxItem, MailboxesConfig, OptionalItemTraits,
    SyncStatus,
};
use tokio::sync::{Mutex, watch};
use tokio::time::Instant;

/// Per-mailbox handle owning the client, its delivery engine, and its
/// connection state. Held inside `Mailboxes` as `Arc<TrackedMailbox<...>>` so
/// cheap clones can be handed out to subscribers. The client and its
/// `OperationDelivery` are held behind one `Mutex` so re-registering a
/// mailbox can swap both in place (e.g. mDNS re-resolution producing a new
/// URL) without disturbing the `health` watcher's existing
/// subscribers — delivery never observes a client/engine mismatch.
/// `MailboxSession` is the swappable client-plus-engines bundle;
/// `MailboxConnectionHealth` is the health bookkeeping watching over it.
pub struct TrackedMailbox<Item, Store>
where
    Item: MailboxItem,
    Store: MailboxStore<Item>,
{
    inner: Mutex<MailboxSession<Item, Store>>,
    pub(crate) health: watch::Sender<MailboxConnectionHealth>,
    polling: AtomicBool,
    pending_request: std::sync::Mutex<PendingRequest<Item::Topic>>,
    config: MailboxesConfig,
}

struct MailboxSession<Item, Store>
where
    Item: MailboxItem,
    Store: MailboxStore<Item>,
{
    client: Arc<dyn MailboxClient<Item>>,
    delivery: OperationDelivery<Item, Store>,
}

impl<Item, Store> TrackedMailbox<Item, Store>
where
    Item: MailboxItem,
    Store: MailboxStore<Item>,
    Item::Topic: OptionalItemTraits + Display,
{
    pub(crate) fn new(
        client: Arc<dyn MailboxClient<Item>>,
        store: Store,
        config: MailboxesConfig,
        initial_status: SyncStatus,
    ) -> Self {
        let (health, _) = watch::channel(MailboxConnectionHealth::seeded(initial_status, &config));
        let delivery = OperationDelivery::new(client.clone(), store);
        Self {
            inner: Mutex::new(MailboxSession { client, delivery }),
            health,
            polling: AtomicBool::new(false),
            pending_request: std::sync::Mutex::new(PendingRequest::default()),
            config,
        }
    }

    /// The delivery engine for this mailbox's current client.
    pub(crate) async fn delivery(&self) -> OperationDelivery<Item, Store> {
        self.inner.lock().await.delivery.clone()
    }

    pub async fn client(&self) -> Arc<dyn MailboxClient<Item>> {
        self.inner.lock().await.client.clone()
    }

    /// Swap the client (re-registration with a fresh URL) and rebuild its
    /// delivery engine. The engine is stateless — durable delivery state
    /// lives in the manager's trackers — so rebuilding loses nothing.
    pub(crate) async fn replace_client(&self, client: Arc<dyn MailboxClient<Item>>, store: Store) {
        let delivery = OperationDelivery::new(client.clone(), store);
        *self.inner.lock().await = MailboxSession { client, delivery };
    }

    pub fn health(&self) -> watch::Receiver<MailboxConnectionHealth> {
        self.health.subscribe()
    }

    pub(crate) fn record_success(&self) {
        self.health.send_modify(|t| t.record_success(&self.config));
    }

    pub(crate) fn record_error(&self, err: String) {
        self.health
            .send_modify(|t| t.record_error(&self.config, err));
    }

    pub(crate) fn record_probe_error(&self, err: String) {
        self.health
            .send_modify(|t| t.record_probe_error(&self.config, err));
    }

    pub(crate) fn reschedule(&self) {
        self.health.send_modify(|t| t.reschedule(&self.config));
    }

    /// Treat this mailbox as healthy again and poll every subscribed topic
    /// immediately.
    pub(crate) fn wakeup(&self) {
        self.pending_request.lock().unwrap().merge_wakeup();
        self.health.send_modify(|t| t.wakeup());
    }

    /// Poll every subscribed topic now without presuming the outcome.
    pub(crate) fn probe(&self) {
        self.pending_request.lock().unwrap().merge_probe();
    }

    /// If active, sync `topic` (or every subscribed topic, when `None`) after
    /// the debounce.
    pub(crate) fn request_sync_if_active(&self, topic: Option<Item::Topic>) {
        if self.health.borrow().status != SyncStatus::Active {
            return;
        }
        self.pending_request.lock().unwrap().merge_poll(topic);
    }

    /// When this mailbox should next be polled: its scheduled poll, or sooner
    /// if a request is pending.
    pub(crate) fn due_at(&self, now: Instant) -> Instant {
        let next_poll = self.health.borrow().next_poll;
        self.pending_request
            .lock()
            .unwrap()
            .due_at(now, next_poll, self.config.sync_debounce)
    }

    /// `probe` says whether the poll that failed was fulfilling a probe. A
    /// probe failing against a mailbox still believed Active is news, not
    /// confirmation, so it counts like any other error.
    pub(crate) fn poll_failed(&self, err: String, probe: bool) {
        let confirms_known_failure = probe && self.health.borrow().status != SyncStatus::Active;
        if confirms_known_failure {
            self.record_probe_error(err);
        } else {
            self.record_error(err);
        }
        // A sync requested during a failed poll doesn't earn a quick retry:
        // the request failed too, so backoff applies. A wakeup or probe does,
        // since each demands a poll now regardless.
        let mut pending = self.pending_request.lock().unwrap();
        if !matches!(*pending, PendingRequest::Wakeup | PendingRequest::Probe) {
            *pending = PendingRequest::default();
        }
    }

    /// Consume the sync request the imminent poll is about to fulfil.
    pub(crate) fn consume_pending_request(&self) -> (RequestOutcome<Item::Topic>, bool) {
        self.pending_request.lock().unwrap().consume()
    }

    /// Take the sync request the imminent poll is about to fulfil, keeping the
    /// variant detail tests assert on.
    #[cfg(test)]
    pub(crate) fn take_pending_request(&self) -> PendingRequest<Item::Topic> {
        std::mem::replace(
            &mut self.pending_request.lock().unwrap(),
            PendingRequest::default(),
        )
    }

    /// Claim this mailbox for a poll, returning `false` if one is already in flight.
    pub(crate) fn begin_poll(&self) -> bool {
        !self.polling.swap(true, Ordering::SeqCst)
    }

    pub(crate) fn end_poll(&self) {
        self.polling.store(false, Ordering::SeqCst);
    }

    pub(crate) fn is_polling(&self) -> bool {
        self.polling.load(Ordering::SeqCst)
    }
}
