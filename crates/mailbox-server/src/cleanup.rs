use redb::{Database, ReadableTable};
use std::sync::Arc;
use std::time::Duration;

use crate::{BlobsKey, BLOBS_TABLE};

const CLEANUP_INTERVAL: Duration = Duration::from_secs(5 * 60); // 5 minutes
const MESSAGE_MAX_AGE: Duration = Duration::from_secs(7 * 24 * 60 * 60); // 7 days

/// Spawns a background task that periodically cleans up old messages
pub fn spawn_cleanup_task(db: Arc<Database>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(CLEANUP_INTERVAL);

        loop {
            interval.tick().await;

            if let Err(e) = cleanup_old_messages(&db).await {
                tracing::error!("Failed to cleanup old messages: {}", e);
            }
        }
    })
}

/// Deletes all messages older than MESSAGE_MAX_AGE
pub async fn cleanup_old_messages(db: &Database) -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("Starting cleanup of old messages");

    let cutoff_time = std::time::SystemTime::now() - MESSAGE_MAX_AGE;
    let cutoff_uuid = uuid::Uuid::new_v7(uuid::Timestamp::from_unix(
        uuid::NoContext,
        cutoff_time.duration_since(std::time::UNIX_EPOCH)?.as_secs(),
        0,
    ));

    let write_txn = db.begin_write()?;
    let mut deleted_count = 0;

    {
        let mut table = write_txn.open_table(BLOBS_TABLE)?;

        // Collect keys to delete
        let mut keys_to_delete: Vec<BlobsKey> = Vec::new();

        for entry in table.iter()? {
            let (key, _value) = entry?;
            let blob_key: BlobsKey = key.value();

            if blob_key.uuid < cutoff_uuid {
                keys_to_delete.push(blob_key);
            }
        }

        // Delete old messages
        for key in &keys_to_delete {
            table.remove(key)?;
            deleted_count += 1;
        }
    }

    write_txn.commit()?;

    tracing::info!("Cleanup completed: deleted {} old messages", deleted_count);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use redb::ReadableDatabase;
    use tempfile::NamedTempFile;

    fn create_test_db() -> (Database, NamedTempFile) {
        let temp_file = NamedTempFile::new().unwrap();
        let db = Database::create(temp_file.path()).unwrap();

        let write_txn = db.begin_write().unwrap();
        {
            let _table = write_txn.open_table(BLOBS_TABLE).unwrap();
        }
        write_txn.commit().unwrap();

        (db, temp_file)
    }

    #[tokio::test]
    async fn test_cleanup_old_messages() {
        let (db, _temp_file) = create_test_db();

        // Insert an old message (8 days ago)
        let old_time = std::time::SystemTime::now() - Duration::from_secs(8 * 24 * 60 * 60);
        let old_uuid = uuid::Uuid::new_v7(uuid::Timestamp::from_unix(
            uuid::NoContext,
            old_time
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            0,
        ));
        let old_key = BlobsKey::new("test-topic".into(), "log-1".into(), 0, old_uuid).unwrap();

        // Insert a recent message (1 day ago)
        let recent_uuid = uuid::Uuid::now_v7();
        let recent_key =
            BlobsKey::new("test-topic".into(), "log-1".into(), 1, recent_uuid).unwrap();

        {
            let write_txn = db.begin_write().unwrap();
            {
                let mut table = write_txn.open_table(BLOBS_TABLE).unwrap();
                table.insert(&old_key, b"old message".as_slice()).unwrap();
                table
                    .insert(&recent_key, b"recent message".as_slice())
                    .unwrap();
            }
            write_txn.commit().unwrap();
        }

        // Verify both messages exist
        {
            let read_txn = db.begin_read().unwrap();
            let table = read_txn.open_table(BLOBS_TABLE).unwrap();
            assert!(table.get(&old_key).unwrap().is_some());
            assert!(table.get(&recent_key).unwrap().is_some());
        }

        // Run cleanup
        cleanup_old_messages(&db).await.unwrap();

        // Verify old message is deleted and recent message remains
        {
            let read_txn = db.begin_read().unwrap();
            let table = read_txn.open_table(BLOBS_TABLE).unwrap();
            assert!(table.get(&old_key).unwrap().is_none());
            assert!(table.get(&recent_key).unwrap().is_some());
        }
    }
}
