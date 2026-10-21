pub use serde::{Serialize, de::DeserializeOwned};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    marker::PhantomData,
    sync::Arc,
};
use tokio::sync::{Mutex, watch};

use crate::MailboxId;
use crate::SeqNum;
use crate::connection_health::SyncStatus;
use crate::persistence::{ByteDeserializable, ByteSerializable, MailboxStore, SyncStore};

/// Per-mailbox sync watermarks: `topic -> author -> highest seq num the mailbox holds`.
pub type MailboxSyncState<T, A> = HashMap<T, HashMap<A, SeqNum>>;

/// Persistent, watch-based tracker for what each mailbox has acknowledged syncing.
/// SQLite-backed (or in-memory for tests), with watch channels layered on top so
/// callers can subscribe to live updates.
pub struct MailboxSyncTracker<T, A> {
    store: Arc<dyn SyncStore<T, A>>,
    mailbox_store: Arc<dyn MailboxStore>,
    all_ids_tx: watch::Sender<BTreeSet<MailboxId>>,
    per_mailbox: Mutex<HashMap<MailboxId, watch::Sender<MailboxSyncState<T, A>>>>,
    _phantom: PhantomData<fn() -> (T, A)>,
}

impl<T, A> MailboxSyncTracker<T, A>
where
    T: ByteSerializable
        + ByteDeserializable<T>
        + Eq
        + std::hash::Hash
        + Clone
        + Send
        + Sync
        + 'static,
    A: ByteSerializable
        + ByteDeserializable<A>
        + Eq
        + std::hash::Hash
        + Clone
        + Send
        + Sync
        + 'static,
{
    pub fn new(
        store: Arc<dyn SyncStore<T, A>>,
        mailbox_store: Arc<dyn MailboxStore>,
        initial_ids: BTreeSet<MailboxId>,
    ) -> Self {
        let (all_ids_tx, _) = watch::channel(initial_ids);
        Self {
            store,
            mailbox_store,
            all_ids_tx,
            per_mailbox: Mutex::new(HashMap::new()),
            _phantom: PhantomData,
        }
    }

    /// Subscribe to the set of mailbox ids that have ever recorded sync state.
    pub fn all_mailbox_ids(&self) -> watch::Receiver<BTreeSet<MailboxId>> {
        self.all_ids_tx.subscribe()
    }

    /// Subscribe to per-mailbox sync watermarks. Lazily creates the watch on
    /// first call, seeded from persisted state.
    pub async fn sync_state(
        &self,
        mailbox: &MailboxId,
    ) -> anyhow::Result<watch::Receiver<MailboxSyncState<T, A>>> {
        let mut per_mailbox = self.per_mailbox.lock().await;
        if let Some(tx) = per_mailbox.get(mailbox) {
            return Ok(tx.subscribe());
        }
        let initial = self.get_all_for_mailbox(mailbox).await?;
        let (tx, rx) = watch::channel(initial);
        per_mailbox.insert(mailbox.clone(), tx);
        Ok(rx)
    }

    /// Record a batch of `(topic, author, seq)` watermarks for one mailbox in
    /// a single SQL statement (multi-row INSERT with upsert). Updates the
    /// per-mailbox sync-state watch (if subscribed) and the all-ids watch
    /// when a new mailbox is observed.
    pub async fn record_synced(
        &self,
        mailbox: &MailboxId,
        entries: &[(T, A, SeqNum)],
    ) -> anyhow::Result<()> {
        if entries.is_empty() {
            return Ok(());
        }

        self.store.record_synced(mailbox, entries).await?;

        self.all_ids_tx
            .send_if_modified(|ids| ids.insert(mailbox.clone()));

        let per_mailbox = self.per_mailbox.lock().await;
        if let Some(tx) = per_mailbox.get(mailbox) {
            tx.send_if_modified(|state| {
                let mut changed = false;
                for (t, a, s) in entries {
                    let map = state.entry(t.clone()).or_default();
                    match map.get_mut(a) {
                        Some(entry) if *s > *entry => {
                            *entry = *s;
                            changed = true;
                        }
                        None => {
                            map.insert(a.clone(), *s);
                            changed = true;
                        }
                        _ => {}
                    }
                }
                changed
            });
        }
        Ok(())
    }
    /// Persist the base URL a mailbox is reached at, so the mailbox can later be
    /// identified by URL even when it is not currently registered (e.g. the
    /// cloud mailbox after a cold start while its server is unreachable).
    pub async fn record_url(&self, mailbox: &MailboxId, url: &str) -> anyhow::Result<()> {
        self.mailbox_store.record_url(mailbox, url).await?;
        Ok(())
    }

    /// Persist the last judged sync status for a mailbox, so a later
    /// registration (e.g. after a restart or an iOS node rebuild) can seed its
    /// tracker from history instead of assuming the mailbox is healthy.
    pub async fn record_status(
        &self,
        mailbox: &MailboxId,
        status: SyncStatus,
    ) -> anyhow::Result<()> {
        self.store.record_status(mailbox, status).await?;
        Ok(())
    }

    /// The last recorded sync status for a mailbox, if any.
    pub async fn get_status(&self, mailbox: &MailboxId) -> anyhow::Result<Option<SyncStatus>> {
        self.store.get_status(mailbox).await
    }

    /// Forget the persisted status for a mailbox, so its next registration
    /// starts from Active.
    pub async fn clear_status(&self, mailbox: &MailboxId) -> anyhow::Result<()> {
        self.store.clear_status(mailbox).await?;
        Ok(())
    }

    /// The id of the mailbox last recorded at `url`, if any.
    pub async fn mailbox_id_for_url(&self, url: &str) -> anyhow::Result<Option<MailboxId>> {
        self.mailbox_store.get_url_inverse(url).await
    }

    pub async fn get_synced(
        &self,
        mailbox: &MailboxId,
        topic: &T,
        author: &A,
    ) -> anyhow::Result<Option<SeqNum>> {
        self.store.get_watermark(mailbox, topic, author).await
    }

    /// All `(mailbox_id, seq)` entries for the given (topic, author).
    pub async fn get_synced_for_log(
        &self,
        topic: &T,
        author: &A,
    ) -> anyhow::Result<BTreeMap<MailboxId, SeqNum>> {
        self.store.get_synced_for_log(topic, author).await
    }

    /// All `topic -> author -> seq` entries for the given mailbox.
    pub async fn get_all_for_mailbox(
        &self,
        mailbox: &MailboxId,
    ) -> anyhow::Result<MailboxSyncState<T, A>> {
        let state = self.store.get_sync_state(mailbox).await?;
        Ok(state)
    }

    pub async fn drop_mailbox(&self, mailbox: &MailboxId) -> anyhow::Result<()> {
        self.store.drop_mailbox(mailbox).await?;
        self.all_ids_tx.send_if_modified(|ids| ids.remove(mailbox));
        self.per_mailbox.lock().await.remove(mailbox);
        Ok(())
    }
}

pub fn encode<T: Serialize>(value: &T) -> anyhow::Result<Vec<u8>> {
    let mut buf = Vec::new();
    ciborium::into_writer(value, &mut buf)?;
    Ok(buf)
}

pub fn decode<T: DeserializeOwned>(bytes: &[u8]) -> anyhow::Result<T> {
    Ok(ciborium::from_reader(bytes)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistence::Persistence;
    use crate::persistence::sqlite::SqlitePersistence;
    use pretty_assertions::assert_eq;

    enum Backend {
        Sqlite,
        Mem,
    }

    async fn open(backend: Backend) -> (Option<tempfile::TempDir>, MailboxSyncTracker<u8, char>) {
        match backend {
            Backend::Sqlite => {
                let dir = tempfile::tempdir().unwrap();
                let path = dir.path().join("sync_state.db");
                let persistence =
                    SqlitePersistence::<u8, char>::open(&path, &sqlite_gate::SqliteGate::default())
                        .await
                        .unwrap();
                let store = MailboxSyncTracker::<u8, char>::new(
                    persistence.sync_store(),
                    persistence.mailbox_store(),
                    persistence.sync_store().load_all_ids().await.unwrap(),
                );
                (Some(dir), store)
            }
            Backend::Mem => {
                // Mock-time-safe in-memory backend; a sqlx :memory: pool would
                // deadlock under tokio::test(start_paused = true).
                let persistence = crate::persistence::mem::MemPersistence::<u8, char>::new();
                let store = MailboxSyncTracker::<u8, char>::new(
                    persistence.sync_store(),
                    persistence.mailbox_store(),
                    BTreeSet::new(),
                );
                (None, store)
            }
        }
    }

    async fn round_trip_impl(b: Backend) {
        let (_dir, store) = open(b).await;
        let mailbox = "mb1".to_string();
        store
            .record_synced(&mailbox, &[(7u8, 'a', 3)])
            .await
            .unwrap();
        let got = store.get_synced(&mailbox, &7u8, &'a').await.unwrap();
        assert_eq!(got, Some(3));
    }

    #[tokio::test]
    async fn round_trip_sqlite() {
        round_trip_impl(Backend::Sqlite).await;
    }

    #[tokio::test]
    async fn round_trip_mem() {
        round_trip_impl(Backend::Mem).await;
    }

    async fn monotonic_impl(b: Backend) {
        let (_dir, store) = open(b).await;
        let mb = "mb1".to_string();
        store.record_synced(&mb, &[(7u8, 'a', 5)]).await.unwrap();
        store.record_synced(&mb, &[(7u8, 'a', 3)]).await.unwrap();
        assert_eq!(store.get_synced(&mb, &7u8, &'a').await.unwrap(), Some(5));
        store.record_synced(&mb, &[(7u8, 'a', 10)]).await.unwrap();
        assert_eq!(store.get_synced(&mb, &7u8, &'a').await.unwrap(), Some(10));
    }

    #[tokio::test]
    async fn monotonic_sqlite() {
        monotonic_impl(Backend::Sqlite).await;
    }

    #[tokio::test]
    async fn monotonic_mem() {
        monotonic_impl(Backend::Mem).await;
    }

    async fn multi_mailbox_impl(b: Backend) {
        let (_dir, store) = open(b).await;
        store
            .record_synced(&"mb1".into(), &[(7u8, 'a', 1)])
            .await
            .unwrap();
        store
            .record_synced(&"mb2".into(), &[(7u8, 'a', 5)])
            .await
            .unwrap();
        let for_log = store.get_synced_for_log(&7u8, &'a').await.unwrap();
        assert_eq!(for_log.get("mb1"), Some(&1));
        assert_eq!(for_log.get("mb2"), Some(&5));
    }

    #[tokio::test]
    async fn multi_mailbox_sqlite() {
        multi_mailbox_impl(Backend::Sqlite).await;
    }

    #[tokio::test]
    async fn multi_mailbox_mem() {
        multi_mailbox_impl(Backend::Mem).await;
    }

    async fn get_all_for_mailbox_impl(b: Backend) {
        let (_dir, store) = open(b).await;
        store
            .record_synced(&"mb1".into(), &[(7u8, 'a', 1)])
            .await
            .unwrap();
        store
            .record_synced(&"mb1".into(), &[(7u8, 'b', 2)])
            .await
            .unwrap();
        store
            .record_synced(&"mb1".into(), &[(8u8, 'a', 3)])
            .await
            .unwrap();
        store
            .record_synced(&"mb2".into(), &[(7u8, 'a', 99)])
            .await
            .unwrap();
        let all = store.get_all_for_mailbox(&"mb1".into()).await.unwrap();
        assert_eq!(all.values().map(|m| m.len()).sum::<usize>(), 3);
        assert_eq!(all.get(&7u8).and_then(|m| m.get(&'a')), Some(&1));
        assert_eq!(all.get(&7u8).and_then(|m| m.get(&'b')), Some(&2));
        assert_eq!(all.get(&8u8).and_then(|m| m.get(&'a')), Some(&3));
    }

    #[tokio::test]
    async fn get_all_for_mailbox_sqlite() {
        get_all_for_mailbox_impl(Backend::Sqlite).await;
    }

    #[tokio::test]
    async fn get_all_for_mailbox_mem() {
        get_all_for_mailbox_impl(Backend::Mem).await;
    }

    async fn drop_mailbox_impl(b: Backend) {
        let (_dir, store) = open(b).await;
        store
            .record_synced(&"mb1".into(), &[(7u8, 'a', 1)])
            .await
            .unwrap();
        store
            .record_synced(&"mb2".into(), &[(7u8, 'a', 2)])
            .await
            .unwrap();
        store.drop_mailbox(&"mb1".into()).await.unwrap();
        assert_eq!(
            store.get_synced(&"mb1".into(), &7u8, &'a').await.unwrap(),
            None
        );
        assert_eq!(
            store.get_synced(&"mb2".into(), &7u8, &'a').await.unwrap(),
            Some(2)
        );
    }

    #[tokio::test]
    async fn drop_mailbox_sqlite() {
        drop_mailbox_impl(Backend::Sqlite).await;
    }

    #[tokio::test]
    async fn drop_mailbox_mem() {
        drop_mailbox_impl(Backend::Mem).await;
    }

    #[tokio::test]
    async fn persists_across_reopen_sqlite() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sync_state.db");

        {
            let persistence =
                SqlitePersistence::<u8, char>::open(&path, &sqlite_gate::SqliteGate::default())
                    .await
                    .unwrap();
            let store = MailboxSyncTracker::new(
                persistence.sync_store(),
                persistence.mailbox_store(),
                persistence.sync_store().load_all_ids().await.unwrap(),
            );
            store
                .record_synced(&"mb1".into(), &[(7u8, 'a', 3)])
                .await
                .unwrap();
            store
                .record_synced(&"mb1".into(), &[(7u8, 'b', 4)])
                .await
                .unwrap();
            // store.close().await;
        }

        let persistence =
            SqlitePersistence::<u8, char>::open(&path, &sqlite_gate::SqliteGate::default())
                .await
                .unwrap();
        let store = MailboxSyncTracker::new(
            persistence.sync_store(),
            persistence.mailbox_store(),
            persistence.sync_store().load_all_ids().await.unwrap(),
        );
        assert_eq!(
            store.get_synced(&"mb1".into(), &7u8, &'a').await.unwrap(),
            Some(3),
        );
        assert_eq!(
            store.get_synced(&"mb1".into(), &7u8, &'b').await.unwrap(),
            Some(4),
        );
        // Removed the check for mb2 as it was never recorded in this test.

        let all = store.get_all_for_mailbox(&"mb1".into()).await.unwrap();
        assert_eq!(all.values().map(|m| m.len()).sum::<usize>(), 2);
        assert_eq!(all.get(&7u8).and_then(|m| m.get(&'a')), Some(&3));
        assert_eq!(all.get(&7u8).and_then(|m| m.get(&'b')), Some(&4));

        store
            .record_synced(&"mb1".into(), &[(7u8, 'a', 2)])
            .await
            .unwrap();
        assert_eq!(
            store.get_synced(&"mb1".into(), &7u8, &'a').await.unwrap(),
            Some(3),
        );
        store
            .record_synced(&"mb1".into(), &[(7u8, 'a', 10)])
            .await
            .unwrap();
        assert_eq!(
            store.get_synced(&"mb1".into(), &7u8, &'a').await.unwrap(),
            Some(10),
        );

        let ids = store.all_mailbox_ids().borrow().clone();
        assert!(ids.contains("mb1"));
    }

    #[tokio::test]
    async fn all_mailbox_ids_watch_updates_on_record_synced_and_drop() {
        let p = crate::persistence::mem::MemPersistence::<u8, char>::new();
        let store =
            MailboxSyncTracker::<u8, char>::new(p.sync_store(), p.mailbox_store(), BTreeSet::new());
        let mut rx = store.all_mailbox_ids();
        assert!(rx.borrow().is_empty());

        store
            .record_synced(&"mb1".into(), &[(7u8, 'a', 1)])
            .await
            .unwrap();
        rx.changed().await.unwrap();
        assert!(rx.borrow().contains("mb1"));

        store
            .record_synced(&"mb2".into(), &[(7u8, 'a', 1)])
            .await
            .unwrap();
        rx.changed().await.unwrap();
        assert!(rx.borrow().contains("mb2"));

        // Re-recording for an existing mailbox should NOT bump the watch.
        store
            .record_synced(&"mb1".into(), &[(7u8, 'a', 2)])
            .await
            .unwrap();
        assert!(!rx.has_changed().unwrap());

        store.drop_mailbox(&"mb1".into()).await.unwrap();
        rx.changed().await.unwrap();
        assert!(!rx.borrow().contains("mb1"));
        assert!(rx.borrow().contains("mb2"));
    }

    async fn record_status_round_trip_impl(b: Backend) {
        let (_dir, store) = open(b).await;
        assert_eq!(store.get_status(&"mb1".into()).await.unwrap(), None);
        store
            .record_status(&"mb1".into(), SyncStatus::Stopped)
            .await
            .unwrap();
        assert_eq!(
            store.get_status(&"mb1".into()).await.unwrap(),
            Some(SyncStatus::Stopped),
        );
        store
            .record_status(&"mb1".into(), SyncStatus::Active)
            .await
            .unwrap();
        assert_eq!(
            store.get_status(&"mb1".into()).await.unwrap(),
            Some(SyncStatus::Active),
        );
        store.clear_status(&"mb1".into()).await.unwrap();
        assert_eq!(store.get_status(&"mb1".into()).await.unwrap(), None);
        store
            .record_status(&"mb1".into(), SyncStatus::Stopped)
            .await
            .unwrap();
        store.drop_mailbox(&"mb1".into()).await.unwrap();
        assert_eq!(store.get_status(&"mb1".into()).await.unwrap(), None);
    }

    #[tokio::test]
    async fn record_status_round_trip_sqlite() {
        record_status_round_trip_impl(Backend::Sqlite).await;
    }

    #[tokio::test]
    async fn record_status_round_trip_mem() {
        record_status_round_trip_impl(Backend::Mem).await;
    }

    #[tokio::test]
    async fn status_persists_across_reopen_sqlite() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sync_state.db");

        {
            let persistence =
                SqlitePersistence::<u8, char>::open(&path, &sqlite_gate::SqliteGate::default())
                    .await
                    .unwrap();
            let store = MailboxSyncTracker::new(
                persistence.sync_store(),
                persistence.mailbox_store(),
                persistence.sync_store().load_all_ids().await.unwrap(),
            );
            store
                .record_status(&"mb1".into(), SyncStatus::Degraded)
                .await
                .unwrap();
            // store.close().await;
        }

        let persistence =
            SqlitePersistence::<u8, char>::open(&path, &sqlite_gate::SqliteGate::default())
                .await
                .unwrap();
        let store = MailboxSyncTracker::new(
            persistence.sync_store(),
            persistence.mailbox_store(),
            persistence.sync_store().load_all_ids().await.unwrap(),
        );
        assert_eq!(
            store.get_status(&"mb1".into()).await.unwrap(),
            Some(SyncStatus::Degraded),
        );
    }

    async fn record_url_round_trip_impl(b: Backend) {
        let (_dir, store) = open(b).await;
        store
            .record_url(&"mb1".into(), "https://cloud.example")
            .await
            .unwrap();
        assert_eq!(
            store
                .mailbox_id_for_url("https://cloud.example")
                .await
                .unwrap(),
            Some("mb1".to_string()),
        );
        assert_eq!(
            store
                .mailbox_id_for_url("https://other.example")
                .await
                .unwrap(),
            None,
        );
    }

    #[tokio::test]
    async fn record_url_round_trip_sqlite() {
        record_url_round_trip_impl(Backend::Sqlite).await;
    }

    #[tokio::test]
    async fn record_url_round_trip_mem() {
        record_url_round_trip_impl(Backend::Mem).await;
    }
    #[tokio::test]
    async fn sync_state_watch_updates_on_record_synced() {
        let p = crate::persistence::mem::MemPersistence::<u8, char>::new();
        let store =
            MailboxSyncTracker::<u8, char>::new(p.sync_store(), p.mailbox_store(), BTreeSet::new());
        let mut rx = store.sync_state(&"mb1".into()).await.unwrap();
        assert!(rx.borrow().is_empty());

        store
            .record_synced(&"mb1".into(), &[(7u8, 'a', 3)])
            .await
            .unwrap();
        rx.changed().await.unwrap();
        assert_eq!(rx.borrow().get(&7u8).and_then(|m| m.get(&'a')), Some(&3));

        store
            .record_synced(&"mb1".into(), &[(7u8, 'a', 5), (8u8, 'b', 1)])
            .await
            .unwrap();
        rx.changed().await.unwrap();
        let snap = rx.borrow().clone();
        assert_eq!(snap.get(&7u8).and_then(|m| m.get(&'a')), Some(&5));
        assert_eq!(snap.get(&8u8).and_then(|m| m.get(&'b')), Some(&1));
    }
}
