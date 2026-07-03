//! Topic enumeration. The sync engine polls only *subscribed* topics, but a
//! replicating mailbox must cover every topic it knows (and new ones as they
//! appear). Periodically scan the store's distinct topics and subscribe to each
//! new one, wiring its pulled-item channel into a drain task.

use std::sync::Arc;
use std::time::Duration;

use redb::{Database, ReadableDatabase, ReadableTable};

use mailbox_client::manager::Mailboxes;
use mailbox_server::{WatermarksKey, WATERMARKS_TABLE};
use tokio::task::JoinSet;

use crate::item::BlipItem;
use crate::store::RedbBlipStore;

/// Periodically scan the store's distinct topics and subscribe to each new one,
/// wiring its pulled-item channel into a drain task; runs until cancelled.
pub async fn enumerate_topics_loop(
    db: Arc<Database>,
    mailboxes: Mailboxes<BlipItem, RedbBlipStore>,
    self_store_url: String,
    interval: Duration,
) {
    // The per-topic drain tasks are children of this loop: cancelling (dropping)
    // it drops the JoinSet, which aborts them all.
    let mut drains = JoinSet::new();
    let mut ticker = tokio::time::interval(interval);
    loop {
        ticker.tick().await;
        // Reap drains whose topic channel has closed.
        while drains.try_join_next().is_some() {}
        let topics = match distinct_topics(&db).await {
            Ok(topics) => topics,
            Err(err) => {
                tracing::warn!("topic-enum: failed to scan topics: {err}");
                continue;
            }
        };
        let subscribed = mailboxes.subscribed_topics().await;
        for topic in topics {
            if subscribed.contains(&topic) {
                continue;
            }
            match mailboxes.subscribe(topic).await {
                // `Some` means a newly-subscribed topic; drain its pulled items.
                Ok(Some(rx)) => {
                    drains.spawn(crate::drain::drain_loop(rx, self_store_url.clone()));
                }
                // `None` means already subscribed — nothing to do.
                Ok(None) => {}
                Err(err) => tracing::warn!("topic-enum: subscribe failed: {err}"),
            }
        }
    }
}

async fn distinct_topics(db: &Arc<Database>) -> anyhow::Result<Vec<String>> {
    let db = Arc::clone(db);
    tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<String>> {
        let read_txn = db.begin_read()?;
        let table = read_txn.open_table(WATERMARKS_TABLE)?;
        let mut topics = std::collections::BTreeSet::new();
        for entry in table.iter()? {
            let (key, _value) = entry?;
            let key: WatermarksKey = key.value();
            topics.insert(key.topic_id);
        }
        Ok(topics.into_iter().collect())
    })
    .await?
}
