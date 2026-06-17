use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    marker::PhantomData,
    path::Path,
    sync::Arc,
    time::Duration,
};

use anyhow::Context;
use serde::{Serialize, de::DeserializeOwned};
use sqlx::{
    SqlitePool,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
};
use tokio::sync::{Mutex, watch};

use crate::MailboxId;

const SCHEMA: &str = "CREATE TABLE IF NOT EXISTS mailbox_sync_state (
        mailbox_id TEXT NOT NULL,
        topic      BLIP NOT NULL,
        author     BLIP NOT NULL,
        seq_num    INTEGER NOT NULL,
        updated_at INTEGER NOT NULL,
        PRIMARY KEY (mailbox_id, topic, author)
    );
    CREATE INDEX IF NOT EXISTS idx_sync_state_log ON mailbox_sync_state(topic, author);";

/// Per-mailbox sync watermarks: `topic -> author -> highest seq num the mailbox holds`.
pub type MailboxSyncState<T, A> = HashMap<T, HashMap<A, u64>>;

/// Persistent, watch-based tracker for what each mailbox has acknowledged syncing.
/// SQLite-backed (or in-memory for tests), with watch channels layered on top so
/// callers can subscribe to live updates.
pub struct MailboxSyncTracker<T, A> {
    inner: SyncBackend,
    all_ids_tx: watch::Sender<BTreeSet<MailboxId>>,
    per_mailbox: Mutex<HashMap<MailboxId, watch::Sender<MailboxSyncState<T, A>>>>,
    _phantom: PhantomData<fn() -> (T, A)>,
}

#[derive(Clone)]
enum SyncBackend {
    Sqlite(SqlitePool),
    /// In-memory variant for tests that run under `tokio::test(start_paused = true)`,
    /// where sqlx's pool internals deadlock with mock time.
    Mem(Arc<Mutex<MemRows>>),
}

#[derive(Default)]
struct MemRows {
    /// `(mailbox_id, topic_bytes, author_bytes) -> seq`
    rows: BTreeMap<(MailboxId, Vec<u8>, Vec<u8>), u64>,
}

impl<T, A> MailboxSyncTracker<T, A>
where
    T: Serialize + DeserializeOwned + Eq + std::hash::Hash + Clone + Send + Sync + 'static,
    A: Serialize + DeserializeOwned + Eq + std::hash::Hash + Clone + Send + Sync + 'static,
{
    pub async fn open(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let opts = SqliteConnectOptions::new()
            .filename(path.as_ref())
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .busy_timeout(Duration::from_secs(30));
        let pool = SqlitePoolOptions::new().connect_with(opts).await?;
        sqlx::query(SCHEMA).execute(&pool).await?;

        let initial_ids = load_all_ids_sqlite(&pool).await?;
        let (all_ids_tx, _) = watch::channel(initial_ids);
        Ok(Self {
            inner: SyncBackend::Sqlite(pool),
            all_ids_tx,
            per_mailbox: Mutex::new(HashMap::new()),
            _phantom: PhantomData,
        })
    }

    /// In-memory variant for tests that need to avoid sqlx's pool internals
    /// (e.g. tokio mock-time tests where the pool's acquire_timeout would fire).
    pub fn in_memory() -> Self {
        let (all_ids_tx, _) = watch::channel(BTreeSet::new());
        Self {
            inner: SyncBackend::Mem(Arc::new(Mutex::new(MemRows::default()))),
            all_ids_tx,
            per_mailbox: Mutex::new(HashMap::new()),
            _phantom: PhantomData,
        }
    }

    pub async fn close(&self) {
        if let SyncBackend::Sqlite(pool) = &self.inner {
            pool.close().await;
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
        entries: &[(T, A, u64)],
    ) -> anyhow::Result<()> {
        if entries.is_empty() {
            return Ok(());
        }
        let mut encoded: Vec<(Vec<u8>, Vec<u8>, u64)> = Vec::with_capacity(entries.len());
        for (t, a, s) in entries {
            encoded.push((
                encode(t).context("encoding topic")?,
                encode(a).context("encoding author")?,
                *s,
            ));
        }
        match &self.inner {
            SyncBackend::Sqlite(pool) => {
                let now = chrono::Utc::now().timestamp_millis();
                let placeholders = std::iter::repeat("(?, ?, ?, ?, ?)")
                    .take(encoded.len())
                    .collect::<Vec<_>>()
                    .join(", ");
                let sql = format!(
                    "INSERT INTO mailbox_sync_state (mailbox_id, topic, author, seq_num, updated_at)
                     VALUES {placeholders}
                     ON CONFLICT (mailbox_id, topic, author) DO UPDATE SET
                        seq_num = MAX(excluded.seq_num, mailbox_sync_state.seq_num),
                        updated_at = excluded.updated_at"
                );
                let mut query = sqlx::query(&sql);
                for (topic_bytes, author_bytes, seq) in &encoded {
                    query = query
                        .bind(mailbox)
                        .bind(topic_bytes)
                        .bind(author_bytes)
                        .bind(*seq as i64)
                        .bind(now);
                }
                query.execute(pool).await?;
            }
            SyncBackend::Mem(rows) => {
                let mut rows = rows.lock().await;
                for (topic_bytes, author_bytes, seq) in &encoded {
                    let key = (mailbox.clone(), topic_bytes.clone(), author_bytes.clone());
                    let entry = rows.rows.entry(key).or_insert(0);
                    if *seq > *entry {
                        *entry = *seq;
                    }
                }
            }
        }

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

    pub async fn get_synced(
        &self,
        mailbox: &MailboxId,
        topic: &T,
        author: &A,
    ) -> anyhow::Result<Option<u64>> {
        let topic_bytes = encode(topic)?;
        let author_bytes = encode(author)?;
        match &self.inner {
            SyncBackend::Sqlite(pool) => {
                let row: Option<(i64,)> = sqlx::query_as(
                    "SELECT seq_num FROM mailbox_sync_state
                     WHERE mailbox_id = ? AND topic = ? AND author = ?",
                )
                .bind(mailbox)
                .bind(&topic_bytes)
                .bind(&author_bytes)
                .fetch_optional(pool)
                .await?;
                Ok(row.map(|(s,)| s as u64))
            }
            SyncBackend::Mem(rows) => {
                let rows = rows.lock().await;
                Ok(rows
                    .rows
                    .get(&(mailbox.clone(), topic_bytes, author_bytes))
                    .copied())
            }
        }
    }

    /// All `(mailbox_id, seq)` entries for the given (topic, author).
    pub async fn get_synced_for_log(
        &self,
        topic: &T,
        author: &A,
    ) -> anyhow::Result<BTreeMap<MailboxId, u64>> {
        let topic_bytes = encode(topic)?;
        let author_bytes = encode(author)?;
        match &self.inner {
            SyncBackend::Sqlite(pool) => {
                let rows: Vec<(String, i64)> = sqlx::query_as(
                    "SELECT mailbox_id, seq_num FROM mailbox_sync_state
                     WHERE topic = ? AND author = ?",
                )
                .bind(&topic_bytes)
                .bind(&author_bytes)
                .fetch_all(pool)
                .await?;
                Ok(rows.into_iter().map(|(m, s)| (m, s as u64)).collect())
            }
            SyncBackend::Mem(rows) => {
                let rows = rows.lock().await;
                Ok(rows
                    .rows
                    .iter()
                    .filter(|((_, t, a), _)| t == &topic_bytes && a == &author_bytes)
                    .map(|((m, _, _), s)| (m.clone(), *s))
                    .collect())
            }
        }
    }

    /// All `topic -> author -> seq` entries for the given mailbox.
    pub async fn get_all_for_mailbox(
        &self,
        mailbox: &MailboxId,
    ) -> anyhow::Result<MailboxSyncState<T, A>> {
        match &self.inner {
            SyncBackend::Sqlite(pool) => {
                let rows: Vec<(Vec<u8>, Vec<u8>, i64)> = sqlx::query_as(
                    "SELECT topic, author, seq_num FROM mailbox_sync_state
                     WHERE mailbox_id = ?",
                )
                .bind(mailbox)
                .fetch_all(pool)
                .await?;
                let mut out: MailboxSyncState<T, A> = HashMap::new();
                for (t_bytes, a_bytes, s) in rows {
                    let topic: T = decode(&t_bytes).context("decoding topic")?;
                    let author: A = decode(&a_bytes).context("decoding author")?;
                    out.entry(topic).or_default().insert(author, s as u64);
                }
                Ok(out)
            }
            SyncBackend::Mem(rows) => {
                let rows = rows.lock().await;
                let mut out: MailboxSyncState<T, A> = HashMap::new();
                for ((m, t_bytes, a_bytes), s) in rows.rows.iter() {
                    if m == mailbox {
                        let topic: T = decode(t_bytes).context("decoding topic")?;
                        let author: A = decode(a_bytes).context("decoding author")?;
                        out.entry(topic).or_default().insert(author, *s);
                    }
                }
                Ok(out)
            }
        }
    }

    pub async fn drop_mailbox(&self, mailbox: &MailboxId) -> anyhow::Result<()> {
        match &self.inner {
            SyncBackend::Sqlite(pool) => {
                sqlx::query("DELETE FROM mailbox_sync_state WHERE mailbox_id = ?")
                    .bind(mailbox)
                    .execute(pool)
                    .await?;
            }
            SyncBackend::Mem(rows) => {
                let mut rows = rows.lock().await;
                rows.rows.retain(|(m, _, _), _| m != mailbox);
            }
        }
        self.all_ids_tx.send_if_modified(|ids| ids.remove(mailbox));
        self.per_mailbox.lock().await.remove(mailbox);
        Ok(())
    }
}

async fn load_all_ids_sqlite(pool: &SqlitePool) -> anyhow::Result<BTreeSet<MailboxId>> {
    let rows: Vec<(String,)> = sqlx::query_as("SELECT DISTINCT mailbox_id FROM mailbox_sync_state")
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().map(|(m,)| m).collect())
}

fn encode<T: Serialize>(value: &T) -> anyhow::Result<Vec<u8>> {
    let mut buf = Vec::new();
    ciborium::into_writer(value, &mut buf)?;
    Ok(buf)
}

fn decode<T: DeserializeOwned>(bytes: &[u8]) -> anyhow::Result<T> {
    Ok(ciborium::from_reader(bytes)?)
}

#[cfg(test)]
mod tests {
    use super::*;
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
                let store = MailboxSyncTracker::open(&path).await.unwrap();
                (Some(dir), store)
            }
            Backend::Mem => (None, MailboxSyncTracker::in_memory()),
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
            let store: MailboxSyncTracker<u8, char> =
                MailboxSyncTracker::open(&path).await.unwrap();
            store
                .record_synced(&"mb1".into(), &[(7u8, 'a', 3)])
                .await
                .unwrap();
            store
                .record_synced(&"mb1".into(), &[(7u8, 'b', 4)])
                .await
                .unwrap();
            store
                .record_synced(&"mb2".into(), &[(8u8, 'c', 5)])
                .await
                .unwrap();
            store.close().await;
        }

        let store: MailboxSyncTracker<u8, char> = MailboxSyncTracker::open(&path).await.unwrap();
        assert_eq!(
            store.get_synced(&"mb1".into(), &7u8, &'a').await.unwrap(),
            Some(3),
        );
        assert_eq!(
            store.get_synced(&"mb1".into(), &7u8, &'b').await.unwrap(),
            Some(4),
        );
        assert_eq!(
            store.get_synced(&"mb2".into(), &8u8, &'c').await.unwrap(),
            Some(5),
        );

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
        assert!(ids.contains("mb2"));
    }

    #[tokio::test]
    async fn all_mailbox_ids_watch_updates_on_record_synced_and_drop() {
        let store: MailboxSyncTracker<u8, char> = MailboxSyncTracker::in_memory();
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

    #[tokio::test]
    async fn sync_state_watch_updates_on_record_synced() {
        let store: MailboxSyncTracker<u8, char> = MailboxSyncTracker::in_memory();
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
