use mailbox_server::{
    cleanup_old_messages, BlipsKey, GetBlipsResponse, WatermarksKey, BLIPS_TABLE, WATERMARKS_TABLE,
};
use redb::{ReadableDatabase, ReadableTable};
use std::time::Duration;

/// Tests that cleanup of old messages does not affect watermarks or cause
/// the server to re-request blips it already received.
///
/// This is critical for the sync protocol: the watermark represents
/// "we had sequences 0..=watermark at some point" - cleanup should not
/// cause the server to forget this and re-request those sequences.
#[tokio::test]
async fn test_cleanup_preserves_watermark_and_missing_response() {
    // Create test DB
    let (db, _temp_file) = mailbox_server::test_utils::create_test_db();
    let db = std::sync::Arc::new(db);

    let topic = "test-topic";
    let author = "author-1";

    // Step 1: Insert OLD blips directly into DB (8 days ago - will be cleaned up)
    let old_time = std::time::SystemTime::now() - Duration::from_secs(8 * 24 * 60 * 60);
    let old_uuid = uuid::Uuid::new_v7(uuid::Timestamp::from_unix(
        uuid::NoContext,
        old_time
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        0,
    ));

    {
        let write_txn = db.begin_write().unwrap();
        {
            let mut blips = write_txn.open_table(BLIPS_TABLE).unwrap();
            let mut watermarks = write_txn.open_table(WATERMARKS_TABLE).unwrap();

            // Insert old blips (seq 0, 1, 2)
            for seq in 0..=2 {
                let key = BlipsKey::new(topic.into(), author.into(), seq, old_uuid).unwrap();
                blips
                    .insert(&key, format!("old message {}", seq).as_bytes())
                    .unwrap();
            }

            // Insert new blips (seq 3, 4, 5) with current UUID
            let new_uuid = uuid::Uuid::now_v7();
            for seq in 3..=5 {
                let key = BlipsKey::new(topic.into(), author.into(), seq, new_uuid).unwrap();
                blips
                    .insert(&key, format!("new message {}", seq).as_bytes())
                    .unwrap();
            }

            // Set watermark to 5 (sequences 0-5 are contiguous)
            let watermark_key = WatermarksKey::new(topic.into(), author.into()).unwrap();
            watermarks.insert(&watermark_key, 5).unwrap();
        }
        write_txn.commit().unwrap();
    }

    // Step 2: Verify initial state - 6 blips, watermark is 5
    {
        let read_txn = db.begin_read().unwrap();
        let blips = read_txn.open_table(BLIPS_TABLE).unwrap();
        let watermarks = read_txn.open_table(WATERMARKS_TABLE).unwrap();

        let count = blips.iter().unwrap().count();
        assert_eq!(count, 6, "Should have 6 blips initially");

        let watermark_key = WatermarksKey::new(topic.into(), author.into()).unwrap();
        let watermark = watermarks.get(&watermark_key).unwrap().unwrap().value();
        assert_eq!(watermark, 5);
    }

    // Step 3: Run cleanup
    cleanup_old_messages(&db).await.unwrap();

    // Step 4: Verify old blips are deleted, new blips remain
    {
        let read_txn = db.begin_read().unwrap();
        let blips = read_txn.open_table(BLIPS_TABLE).unwrap();

        // Count remaining blips
        let count = blips.iter().unwrap().count();
        assert_eq!(count, 3, "Should have 3 new blips remaining after cleanup");
    }

    // Step 5: Verify watermark is STILL 5 (not reset)
    {
        let read_txn = db.begin_read().unwrap();
        let watermarks = read_txn.open_table(WATERMARKS_TABLE).unwrap();
        let watermark_key = WatermarksKey::new(topic.into(), author.into()).unwrap();
        let watermark = watermarks.get(&watermark_key).unwrap().unwrap().value();
        assert_eq!(watermark, 5, "Watermark should be preserved after cleanup");
    }

    // Step 6: Test get_blips - verify missing response is correct
    let push_tasks = std::sync::Arc::new(tokio::sync::Mutex::new(tokio::task::JoinSet::new()));
    let app = mailbox_server::create_app(db.clone(), None, push_tasks);
    let config = axum_test::TestServerConfig {
        transport: Some(axum_test::Transport::HttpRandomPort),
        ..Default::default()
    };
    let server = axum_test::TestServer::new_with_config(app, config).unwrap();

    let get_response = server
        .post("/blips/get")
        .json(&serde_json::json!({
            "topics": {
                "test-topic": {
                    "author-1": 5  // Client claims to have up to seq 5
                }
            }
        }))
        .await;

    get_response.assert_status_ok();

    let body: GetBlipsResponse = get_response.json();
    let topic_response = &body.blips_by_topic["test-topic"];

    // Missing should be EMPTY - server "had" 0-5 per watermark, doesn't re-request them
    assert!(
        topic_response.missing.is_empty(),
        "Missing should be empty - watermark covers 0-5, cleanup shouldn't cause re-requests. Got: {:?}",
        topic_response.missing
    );

    // Blips should be empty - client already has up to 5, no new blips to send
    assert!(
        topic_response.blips.is_empty()
            || topic_response
                .blips
                .get("author-1")
                .map_or(true, |b| b.is_empty()),
        "No blips should be returned - client already has everything. Got: {:?}",
        topic_response.blips
    );
}
