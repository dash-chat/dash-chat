use anyhow::{Context, Result};
use redb::{Database, ReadableDatabase, TableDefinition};
use std::path::Path;
use std::sync::Arc;

use crate::{
    driver::Driver,
    types::{FcmToken, PublicKey},
};

const FCM_TOKENS: TableDefinition<&str, &str> = TableDefinition::new("fcm_tokens");

pub struct RedbDriver {
    db: Arc<Database>,
}

impl RedbDriver {
    pub fn new(path: &Path) -> Result<Self> {
        // Create parent directory if it does not exist
        if let Some(parent) = path.parent().filter(|p| !p.exists()) {
            std::fs::create_dir_all(parent)?;
        }
        let db = Database::create(path).context("failed to open redb database")?;

        // Ensure the table exists.
        let txn = db.begin_write()?;
        txn.open_table(FCM_TOKENS)?;
        txn.commit()?;

        Ok(Self { db: Arc::new(db) })
    }
}

#[async_trait::async_trait]
impl Driver for RedbDriver {
    async fn store_fcm_token(&self, public_key: &PublicKey, fcm_token: &FcmToken) -> Result<()> {
        let db = self.db.clone();
        let pk = public_key.to_string();
        let token = fcm_token.to_string();

        // redb operations are synchronous and can block on disk I/O,
        // so we run them off the tokio runtime to avoid stalling other tasks.
        tokio::task::spawn_blocking(move || {
            let txn = db.begin_write()?;
            {
                let mut table = txn.open_table(FCM_TOKENS)?;
                table.insert(pk.as_str(), token.as_str())?;
            }
            txn.commit()?;
            Ok(())
        })
        .await
        .context("blocking task panicked")?
    }

    async fn get_fcm_token(&self, public_key: &PublicKey) -> Result<Option<FcmToken>> {
        let db = self.db.clone();
        let pk = public_key.to_string();

        // redb operations are synchronous and can block on disk I/O,
        // so we run them off the tokio runtime to avoid stalling other tasks.
        tokio::task::spawn_blocking(move || -> Result<Option<FcmToken>> {
            let txn = db.begin_read()?;
            let table = txn.open_table(FCM_TOKENS)?;
            let value = table.get(pk.as_str())?;
            Ok(value.map(|v| FcmToken::from(v.value().to_string())))
        })
        .await
        .context("blocking task panicked")?
    }
}
