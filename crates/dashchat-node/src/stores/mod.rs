mod author_store;
mod group_store;
mod local_store;
mod op_store;

pub use author_store::*;
pub use group_store::*;
pub use local_store::*;
pub use op_store::*;
use p2panda_store::SqliteStore;

pub async fn new_sqlite(
    database_file_path: impl AsRef<std::path::Path>,
) -> anyhow::Result<SqliteStore> {
    let path = database_file_path.as_ref().to_path_buf();
    let url = format!("sqlite://{}", path.to_string_lossy());
    p2panda_store::sqlite::create_database(&url).await?;

    let pool = sqlx::SqlitePool::connect(&url)
        .await
        .map_err(|e| anyhow::anyhow!("failed to connect to sqlite at '{path:?}': {e}"))?;

    if p2panda_store::sqlite::run_pending_migrations(&pool)
        .await
        .is_err()
    {
        pool.close().await;
        panic!("Database migration failed");
    }
    let store = SqliteStore::from_pool(pool);
    Ok(store)
}

pub async fn temporary_sqlite() -> anyhow::Result<SqliteStore> {
    let store = SqliteStore::temporary().await;
    Ok(store)
}
