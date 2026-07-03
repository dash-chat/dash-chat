//! A `MailboxStore` over the mailbox's redb blips store, so the `mailbox-client`
//! sync engine can read local watermarks (to build fetch requests) and read
//! local blips (to push to peers that are missing them).

use std::sync::Arc;

use async_trait::async_trait;
use mailbox_client::store::MailboxStore;
use redb::Database;

use crate::item::BlipItem;

#[derive(Clone)]
pub struct RedbBlipStore {
    db: Arc<Database>,
}

impl RedbBlipStore {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }
}

#[async_trait]
impl MailboxStore<BlipItem> for RedbBlipStore {
    async fn get_log(
        &self,
        author: &String,
        topic: &String,
        from: u64,
    ) -> Result<Option<Vec<BlipItem>>, anyhow::Error> {
        let db = Arc::clone(&self.db);
        let topic = topic.clone();
        let author = author.clone();
        let rows = tokio::task::spawn_blocking(move || {
            mailbox_server::blips_since(&db, &topic, &author, from)
        })
        .await??;
        if rows.is_empty() {
            return Ok(None);
        }
        Ok(Some(
            rows.into_iter()
                .map(|(key, blip)| BlipItem { key, blip })
                .collect(),
        ))
    }

    async fn get_log_heights(&self, topic: &String) -> Result<Vec<(String, u64)>, anyhow::Error> {
        let db = Arc::clone(&self.db);
        let topic = topic.clone();
        let heights =
            tokio::task::spawn_blocking(move || mailbox_server::log_heights_for_topic(&db, &topic))
                .await??;
        Ok(heights)
    }
}
