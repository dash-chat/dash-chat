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

use sqlx::{
    SqlitePool,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
};
use std::path::Path;
use std::time::Duration;

pub async fn create_sqlite_pool(path: impl AsRef<Path>) -> anyhow::Result<SqlitePool> {
    let path = path.as_ref().to_path_buf();
    let opts = SqliteConnectOptions::new()
        .filename(&path)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .busy_timeout(Duration::from_secs(30));
    let pool = SqlitePoolOptions::new().connect_with(opts).await?;
    Ok(pool)
}
