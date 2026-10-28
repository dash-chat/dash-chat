use crate::persistence::{MailboxUrlStore, Persistence, SyncStore, decode, encode};
use anyhow::Context;
use async_trait::async_trait;
use mailbox_client::{ItemTraits, MailboxId, SeqNum, SyncStatus};
use sqlite_gate::GatedPool;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

/// DB encoding of `SyncStatus`; these strings are on-disk data and must not
/// change.
fn status_to_db(status: SyncStatus) -> &'static str {
    match status {
        SyncStatus::Active => "active",
        SyncStatus::Degraded => "degraded",
        SyncStatus::Stopped => "stopped",
    }
}

fn status_from_db(s: &str) -> Option<SyncStatus> {
    match s {
        "active" => Some(SyncStatus::Active),
        "degraded" => Some(SyncStatus::Degraded),
        "stopped" => Some(SyncStatus::Stopped),
        _ => None,
    }
}

pub struct SqlitePersistence<T, A> {
    pool: GatedPool,
    mailbox_store: Arc<dyn MailboxUrlStore>,
    sync_store: Arc<dyn SyncStore<T, A>>,
    blob_store: Arc<SqliteBlobStore>,
}

impl<T, A> SqlitePersistence<T, A>
where
    T: ItemTraits + 'static,
    A: ItemTraits + 'static,
{
    pub async fn open(
        path: impl AsRef<std::path::Path>,
        gate: &sqlite_gate::SqliteGate,
    ) -> anyhow::Result<Self> {
        let opts = SqliteConnectOptions::new()
            .filename(path.as_ref())
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .busy_timeout(sqlite_gate::BUSY_TIMEOUT);
        let pool = gate.connect(SqlitePoolOptions::new(), opts).await?;

        sqlx::query(SCHEMA).execute(pool.pool()).await?;

        let mailbox_store = Arc::new(SqliteMailboxUrlStore { pool: pool.clone() });
        let sync_store = Arc::new(SqliteSyncStore::<T, A> {
            pool: pool.clone(),
            _phantom: std::marker::PhantomData,
        });
        let blob_store = Arc::new(SqliteBlobStore { pool: pool.clone() });

        Ok(Self {
            pool,
            blob_store,
            mailbox_store,
            sync_store,
        })
    }

    /// Access the underlying SQLite pool for shutdown.
    #[cfg(test)]
    pub(crate) fn pool(&self) -> &sqlx::SqlitePool {
        self.pool.pool()
    }
}

#[async_trait]
impl<T, A> Persistence<T, A> for SqlitePersistence<T, A>
where
    T: ItemTraits + 'static,
    A: ItemTraits + 'static,
{
    fn mailbox_store(&self) -> Arc<dyn MailboxUrlStore> {
        self.mailbox_store.clone()
    }
    fn sync_store(&self) -> Arc<dyn SyncStore<T, A>> {
        self.sync_store.clone()
    }

    fn blob_store(&self) -> Arc<dyn crate::persistence::BlobStore> {
        self.blob_store.clone()
    }

    async fn close(&self) {
        self.pool.pool().close().await;
    }
}

struct SqliteMailboxUrlStore {
    pool: GatedPool,
}

#[async_trait]
impl MailboxUrlStore for SqliteMailboxUrlStore {
    async fn record_url(&self, id: &MailboxId, url: &str) -> anyhow::Result<()> {
        sqlx::query(
            "INSERT INTO mailbox_url (mailbox_id, url) VALUES (?, ?)
             ON CONFLICT (mailbox_id) DO UPDATE SET url = excluded.url",
        )
        .bind(id)
        .bind(url)
        .execute(self.pool.pool())
        .await?;
        Ok(())
    }
    async fn get_url_inverse(&self, url: &str) -> anyhow::Result<Option<MailboxId>> {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT mailbox_id FROM mailbox_url WHERE url = ? LIMIT 1")
                .bind(url)
                .fetch_optional(self.pool.pool())
                .await?;
        Ok(row.map(|(m,)| m))
    }

    async fn drop_mailbox(&self, id: &MailboxId) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM mailbox_url WHERE mailbox_id = ?")
            .bind(id)
            .execute(self.pool.pool())
            .await?;
        Ok(())
    }
}

struct SqliteSyncStore<T, A> {
    pool: GatedPool,
    _phantom: std::marker::PhantomData<(T, A)>,
}

#[async_trait]
impl<T, A> SyncStore<T, A> for SqliteSyncStore<T, A>
where
    T: ItemTraits + 'static,
    A: ItemTraits + 'static,
{
    async fn record_status(&self, id: &MailboxId, status: SyncStatus) -> anyhow::Result<()> {
        sqlx::query(
            "INSERT INTO mailbox_status (mailbox_id, status, updated_at) VALUES (?, ?, ?)
             ON CONFLICT (mailbox_id) DO UPDATE SET
                status = excluded.status,
                updated_at = excluded.updated_at",
        )
        .bind(id)
        .bind(status_to_db(status))
        .bind(chrono::Utc::now().timestamp_millis())
        .execute(self.pool.pool())
        .await?;
        Ok(())
    }
    async fn get_status(&self, id: &MailboxId) -> anyhow::Result<Option<SyncStatus>> {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT status FROM mailbox_status WHERE mailbox_id = ?")
                .bind(id)
                .fetch_optional(self.pool.pool())
                .await?;
        Ok(row.and_then(|(s,)| status_from_db(&s)))
    }

    async fn clear_status(&self, id: &MailboxId) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM mailbox_status WHERE mailbox_id = ?")
            .bind(id)
            .execute(self.pool.pool())
            .await?;
        Ok(())
    }
    async fn record_synced(
        &self,
        id: &MailboxId,
        watermarks: &[(T, A, SeqNum)],
    ) -> anyhow::Result<()> {
        if watermarks.is_empty() {
            return Ok(());
        }
        let mut encoded: Vec<(Vec<u8>, Vec<u8>, SeqNum)> = Vec::with_capacity(watermarks.len());
        for (t, a, s) in watermarks {
            encoded.push((
                encode(t).context("encoding topic")?,
                encode(a).context("encoding author")?,
                *s,
            ));
        }
        let now = chrono::Utc::now().timestamp_millis();
        let mut query = sqlx::QueryBuilder::<sqlx::Sqlite>::new(
            "INSERT INTO mailbox_sync_state (mailbox_id, topic, author, seq_num, updated_at) ",
        );
        query.push_values(&encoded, |mut row, (t_bytes, a_bytes, seq)| {
            row.push_bind(id)
                .push_bind(t_bytes)
                .push_bind(a_bytes)
                .push_bind(i64::from(*seq))
                .push_bind(now);
        });
        query.push(
            " ON CONFLICT (mailbox_id, topic, author) DO UPDATE SET
                seq_num = MAX(excluded.seq_num, mailbox_sync_state.seq_num),
                updated_at = excluded.updated_at",
        );
        query.build().execute(self.pool.pool()).await?;
        Ok(())
    }
    async fn drop_mailbox(&self, id: &MailboxId) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM mailbox_sync_state WHERE mailbox_id = ?")
            .bind(id)
            .execute(self.pool.pool())
            .await?;
        sqlx::query("DELETE FROM mailbox_status WHERE mailbox_id = ?")
            .bind(id)
            .execute(self.pool.pool())
            .await?;
        Ok(())
    }

    async fn get_sync_state(
        &self,
        id: &MailboxId,
    ) -> anyhow::Result<HashMap<T, HashMap<A, SeqNum>>> {
        let rows: Vec<(Vec<u8>, Vec<u8>, i64)> = sqlx::query_as(
            "SELECT topic, author, seq_num FROM mailbox_sync_state WHERE mailbox_id = ?",
        )
        .bind(id)
        .fetch_all(self.pool.pool())
        .await?;

        let mut out: HashMap<T, HashMap<A, SeqNum>> = HashMap::new();
        for (t_bytes, a_bytes, s) in rows {
            let topic = decode(&t_bytes).context("decoding topic")?;
            let author = decode(&a_bytes).context("decoding author")?;
            out.entry(topic)
                .or_default()
                .insert(author, SeqNum::try_from(s)?);
        }
        Ok(out)
    }
    async fn get_watermark(
        &self,
        id: &MailboxId,
        topic: &T,
        author: &A,
    ) -> anyhow::Result<Option<SeqNum>> {
        let t_bytes = encode(topic)?;
        let a_bytes = encode(author)?;
        let row: Option<(i64,)> = sqlx::query_as(
            "SELECT seq_num FROM mailbox_sync_state
             WHERE mailbox_id = ? AND topic = ? AND author = ?",
        )
        .bind(id)
        .bind(&t_bytes)
        .bind(&a_bytes)
        .fetch_optional(self.pool.pool())
        .await?;
        Ok(row.map(|(s,)| SeqNum::try_from(s)).transpose()?)
    }

    async fn load_all_ids(&self) -> anyhow::Result<BTreeSet<MailboxId>> {
        let rows: Vec<(String,)> =
            sqlx::query_as("SELECT DISTINCT mailbox_id FROM mailbox_sync_state")
                .fetch_all(self.pool.pool())
                .await?;
        Ok(rows.into_iter().map(|(id,)| id).collect())
    }

    async fn get_synced_for_log(
        &self,
        topic: &T,
        author: &A,
    ) -> anyhow::Result<std::collections::BTreeMap<MailboxId, SeqNum>> {
        let t_bytes = encode(topic)?;
        let a_bytes = encode(author)?;
        let rows: Vec<(String, i64)> = sqlx::query_as(
            "SELECT mailbox_id, seq_num FROM mailbox_sync_state
             WHERE topic = ? AND author = ?",
        )
        .bind(t_bytes)
        .bind(a_bytes)
        .fetch_all(self.pool.pool())
        .await?;
        rows.into_iter()
            .map(|(m, s)| Ok((m, SeqNum::try_from(s)?)))
            .collect()
    }
}

const SCHEMA: &str = "CREATE TABLE IF NOT EXISTS mailbox_sync_state (
        mailbox_id TEXT NOT NULL,
        topic      BLOB NOT NULL,
        author     BLOB NOT NULL,
        seq_num    INTEGER NOT NULL,
        updated_at INTEGER NOT NULL,
        PRIMARY KEY (mailbox_id, topic, author)
    );
    CREATE INDEX IF NOT EXISTS idx_sync_state_log ON mailbox_sync_state(topic, author);
    CREATE TABLE IF NOT EXISTS mailbox_url (
        mailbox_id TEXT NOT NULL PRIMARY KEY,
        url        TEXT NOT NULL
    );
    CREATE TABLE IF NOT EXISTS mailbox_status (
        mailbox_id TEXT NOT NULL PRIMARY KEY,
        status     TEXT NOT NULL,
        updated_at INTEGER NOT NULL
    );
    CREATE TABLE IF NOT EXISTS mailbox_unfetched_blobs (
        mailbox_id TEXT NOT NULL,
        blob_hash  BLOB NOT NULL,
        PRIMARY KEY (mailbox_id, blob_hash)
    );";

/// Blob hashes are 32 raw bytes — stored directly, no encoding.
#[derive(Clone)]
struct SqliteBlobStore {
    pool: GatedPool,
}

#[async_trait::async_trait]
impl crate::persistence::BlobStore for SqliteBlobStore {
    async fn record(&self, id: &MailboxId, hashes: &[iroh_blobs::Hash]) -> anyhow::Result<()> {
        if hashes.is_empty() {
            return Ok(());
        }
        let mut query = sqlx::QueryBuilder::<sqlx::Sqlite>::new(
            "INSERT OR IGNORE INTO mailbox_unfetched_blobs (mailbox_id, blob_hash) ",
        );
        query.push_values(hashes, |mut row, hash| {
            row.push_bind(id).push_bind(hash.as_bytes().to_vec());
        });
        query.build().execute(self.pool.pool()).await?;
        Ok(())
    }

    async fn remove(&self, id: &MailboxId, hashes: &[iroh_blobs::Hash]) -> anyhow::Result<()> {
        if hashes.is_empty() {
            return Ok(());
        }
        let mut query = sqlx::QueryBuilder::<sqlx::Sqlite>::new(
            "DELETE FROM mailbox_unfetched_blobs WHERE mailbox_id = ",
        );
        query.push_bind(id).push(" AND blob_hash IN (");
        let mut separated = query.separated(", ");
        for hash in hashes {
            separated.push_bind(hash.as_bytes().to_vec());
        }
        separated.push_unseparated(") ");
        query.build().execute(self.pool.pool()).await?;
        Ok(())
    }

    async fn unfetched(&self, id: &MailboxId) -> anyhow::Result<Vec<iroh_blobs::Hash>> {
        let rows: Vec<(Vec<u8>,)> =
            sqlx::query_as("SELECT blob_hash FROM mailbox_unfetched_blobs WHERE mailbox_id = ?")
                .bind(id)
                .fetch_all(self.pool.pool())
                .await?;
        rows.iter()
            .map(|(bytes,)| {
                let arr: [u8; 32] = bytes
                    .as_slice()
                    .try_into()
                    .map_err(|_| anyhow::anyhow!("blob hash is not 32 bytes"))?;
                Ok(iroh_blobs::Hash::from(arr))
            })
            .collect()
    }

    async fn unfetched_ids(&self) -> anyhow::Result<BTreeSet<MailboxId>> {
        let rows: Vec<(String,)> =
            sqlx::query_as("SELECT DISTINCT mailbox_id FROM mailbox_unfetched_blobs")
                .fetch_all(self.pool.pool())
                .await?;
        Ok(rows.into_iter().map(|(id,)| id).collect())
    }

    async fn remove_all_mailboxes(&self, hashes: &[iroh_blobs::Hash]) -> anyhow::Result<()> {
        for hash in hashes {
            sqlx::query("DELETE FROM mailbox_unfetched_blobs WHERE blob_hash = ?")
                .bind(hash.as_bytes().to_vec())
                .execute(self.pool.pool())
                .await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistence::BlobStore;

    async fn blob_store() -> (tempfile::TempDir, Arc<dyn BlobStore>) {
        let dir = tempfile::tempdir().unwrap();
        let persistence = SqlitePersistence::<u8, char>::open(
            dir.path().join("test.db"),
            &sqlite_gate::SqliteGate::default(),
        )
        .await
        .unwrap();
        let store = persistence.blob_store();
        (dir, store)
    }

    /// Record, read back, and remove — the whole lifecycle on real SQL,
    /// with no tracker involved.
    #[tokio::test]
    async fn blob_store_round_trip() {
        let (_dir, store) = blob_store().await;
        let mbx = "mbx".to_string();
        let h1 = iroh_blobs::Hash::new([1; 32]);
        let h2 = iroh_blobs::Hash::new([2; 32]);

        store.record(&mbx, &[h1, h2]).await.unwrap();
        let mut got = store.unfetched(&mbx).await.unwrap();
        got.sort();
        assert_eq!(got, vec![h1, h2]);

        store.remove(&mbx, &[h1, h2]).await.unwrap();
        assert!(store.unfetched(&mbx).await.unwrap().is_empty());
    }

    /// The multi-bind IN clause: removing several hashes in one statement
    /// must remove exactly those hashes and no others.
    #[tokio::test]
    async fn blob_store_multi_hash_remove() {
        let (_dir, store) = blob_store().await;
        let mbx = "mbx".to_string();
        let hashes: Vec<_> = (0u8..5).map(|i| iroh_blobs::Hash::new([i; 32])).collect();

        store.record(&mbx, &hashes).await.unwrap();
        store.remove(&mbx, &[hashes[0], hashes[2], hashes[4]]).await.unwrap();

        let mut got = store.unfetched(&mbx).await.unwrap();
        got.sort();
        let mut want = vec![hashes[1], hashes[3]];
        want.sort();
        assert_eq!(got, want);
    }

    /// Records are idempotent (INSERT OR IGNORE) and isolated per mailbox.
    #[tokio::test]
    async fn blob_store_record_is_idempotent_and_scoped() {
        let (_dir, store) = blob_store().await;
        let h = iroh_blobs::Hash::new([7; 32]);

        store.record(&"a".into(), &[h]).await.unwrap();
        store.record(&"a".into(), &[h]).await.unwrap();
        store.record(&"b".into(), &[h]).await.unwrap();

        assert_eq!(store.unfetched(&"a".into()).await.unwrap(), vec![h]);
        assert_eq!(store.unfetched(&"b".into()).await.unwrap(), vec![h]);

        store.remove(&"a".into(), &[h]).await.unwrap();
        assert!(store.unfetched(&"a".into()).await.unwrap().is_empty());
        assert_eq!(store.unfetched(&"b".into()).await.unwrap(), vec![h]);
    }

    #[tokio::test]
    async fn gate_still_tracks_pool_after_open() {
        let dir = tempfile::tempdir().unwrap();
        let gate = sqlite_gate::SqliteGate::default();
        let persistence = SqlitePersistence::<u8, char>::open(&dir.path().join("test.db"), &gate)
            .await
            .unwrap();

        // With a connection checked out, suspend must hit its deadline rather
        // than report the pool drained.
        let conn = persistence.pool().acquire().await.unwrap();
        let result = gate.suspend(std::time::Duration::from_millis(300)).await;
        assert!(result.is_err());
        drop(conn);
    }
}
