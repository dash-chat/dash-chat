mod author_store;
mod group_store;
mod local_store;
mod op_projection;
mod op_store;

pub use author_store::*;
pub use group_store::*;
pub use local_store::*;
pub use op_projection::*;
pub use op_store::*;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use std::path::Path;
use std::str::FromStr;
use std::time::Duration;

use sqlite_gate::{GatedPool, SqliteGate};

pub async fn create_sqlite_pool(
    path: impl AsRef<Path>,
    gate: &SqliteGate,
) -> anyhow::Result<GatedPool> {
    let path = path.as_ref().to_path_buf();
    let opts = SqliteConnectOptions::new()
        .filename(&path)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .busy_timeout(Duration::from_secs(30));
    let pool = gate.connect(SqlitePoolOptions::new(), opts).await?;
    Ok(pool)
}

/// Built like p2panda-store's `SqliteStoreBuilder` builds it (settings copied
/// from there), because only a pool connected through `gate` can be suspended.
pub async fn create_op_store_pool(url: &str, gate: &SqliteGate) -> anyhow::Result<GatedPool> {
    p2panda_store::sqlite::create_database(url).await?;
    let opts = SqliteConnectOptions::from_str(url)?.busy_timeout(Duration::from_secs(10));
    let pool = gate
        .connect(
            SqlitePoolOptions::new()
                .min_connections(3)
                .max_connections(16)
                .idle_timeout(Duration::from_secs(10 * 60))
                .max_lifetime(Duration::from_secs(30 * 60)),
            opts,
        )
        .await?;
    p2panda_store::sqlite::run_pending_migrations(pool.pool()).await?;
    Ok(pool)
}
