use std::{sync::Arc, time::Duration};

use axum_test::{TestServer, TestServerConfig, Transport};
use redb::Database;
use tempfile::NamedTempFile;
use tokio::task::JoinSet;

use crate::{
    create_app, Blip, BlobSync, GetBlipsRequest, SeqNum, StoreBlipsRequest, BLIPS_TABLE,
    TOPIC_LOGS_TABLE, WATERMARKS_TABLE,
};

pub const LONG_AGO: Duration = Duration::from_hours(100 * 24); // 100 days

pub fn create_test_db() -> (Database, NamedTempFile) {
    let temp_file = NamedTempFile::new().unwrap();
    let db = Database::create(temp_file.path()).unwrap();

    let write_txn = db.begin_write().unwrap();
    {
        let _blips_table = write_txn.open_table(BLIPS_TABLE).unwrap();
        let _watermarks_table = write_txn.open_table(WATERMARKS_TABLE).unwrap();
        let _topic_logs_table = write_txn.open_table(TOPIC_LOGS_TABLE).unwrap();
    }
    write_txn.commit().unwrap();

    (db, temp_file)
}

pub async fn test_blob_sync() -> BlobSync {
    let dir = tempfile::tempdir().expect("tempdir");
    let key = iroh::SecretKey::generate();
    let bs = BlobSync::new(
        key,
        dir.path().to_path_buf(),
        None,
        *dashchat_utils::NETWORK_ID,
    )
    .await
    .expect("blob sync");
    std::mem::forget(dir);
    bs
}

/// Creates a test server with HTTP transport so server_address() works
pub async fn create_test_server() -> (TestServer, NamedTempFile) {
    let (db, temp_file) = create_test_db();
    let push_tasks = Arc::new(tokio::sync::Mutex::new(JoinSet::new()));
    let blob_sync = test_blob_sync().await;
    let app = create_app(Arc::new(db), None, push_tasks, blob_sync);
    let config = TestServerConfig {
        transport: Some(Transport::HttpRandomPort),
        ..TestServerConfig::default()
    };
    let server = TestServer::new_with_config(app, config).unwrap();
    (server, temp_file)
}

/// A store request of `(topic, author, log, seq, bytes)` entries, associating
/// each log with its topic.
pub fn store_request(entries: &[(&str, &str, &str, SeqNum, &[u8])]) -> StoreBlipsRequest {
    let mut request = StoreBlipsRequest {
        logs: Default::default(),
        blips: Default::default(),
        sender_pubkey: None,
        signature: Vec::new(),
    };
    for (topic, author, log, seq, bytes) in entries {
        request
            .logs
            .entry(topic.to_string())
            .or_default()
            .entry(author.to_string())
            .or_default()
            .insert(log.to_string());
        request
            .blips
            .entry(author.to_string())
            .or_default()
            .entry(log.to_string())
            .or_default()
            .insert(*seq, Blip::new(bytes.to_vec()));
    }
    request
}

/// A get request for `topics`, holding `(author, log, seq)` heights.
pub fn get_request(topics: &[&str], heights: &[(&str, &str, SeqNum)]) -> GetBlipsRequest {
    let mut request = GetBlipsRequest {
        topics: topics.iter().map(|t| t.to_string()).collect(),
        heights: Default::default(),
    };
    for (author, log, seq) in heights {
        request
            .heights
            .entry(author.to_string())
            .or_default()
            .insert(log.to_string(), *seq);
    }
    request
}
