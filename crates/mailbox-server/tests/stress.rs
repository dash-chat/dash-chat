use futures::future::join_all;
use mailbox_server::{
    test_utils::{create_test_server, stored_blip},
    Author, GetBlipsResponse, SequenceNumber, TopicId,
};
use serde_json::json;
use serial_test::serial;
use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Instant;

// Helper to create a simple store request with a single message
fn create_store_request(
    topic_id: &str,
    message: &[u8],
    seq_num: SequenceNumber,
) -> serde_json::Value {
    let message_b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, message);
    json!({
        "blips": {
            topic_id: {
                "author-1": {
                    seq_num.to_string(): stored_blip(&message_b64)
                }
            }
        }
    })
}

// Run this test sequentially to avoid "Too many open files" error when combined with other tests
#[tokio::test]
#[serial]
async fn stress_test_concurrent_writes() {
    let (server, _temp_file) = create_test_server().await;
    let server = Arc::new(server);

    let num_concurrent_writes = 600;
    let num_topics = 10;

    let start = Instant::now();
    let mut tasks = Vec::new();

    for i in 0..num_concurrent_writes {
        let server_clone = Arc::clone(&server);
        let topic_id = format!("stress-topic-{}", i % num_topics);
        let message = format!("Message {}", i);

        let task = async move {
            let response = server_clone
                .post("/blips/store")
                .json(&create_store_request(&topic_id, message.as_bytes(), i))
                .await;

            response.assert_status(axum::http::StatusCode::CREATED);
        };
        tasks.push(task);
    }

    join_all(tasks).await;

    let duration = start.elapsed();

    println!(
        "Concurrent writes: {} messages in {:?} ({:.2} msg/sec)",
        num_concurrent_writes,
        duration,
        num_concurrent_writes as f64 / duration.as_secs_f64()
    );

    // Verify all messages were stored
    for topic_idx in 0..num_topics {
        let topic_id = format!("stress-topic-{}", topic_idx);
        let get_response = server
            .post("/blips/get")
            .json(&json!({
                "topics": {
                    topic_id: {}
                }
            }))
            .await;

        get_response.assert_status_ok();
        let body: GetBlipsResponse = get_response.json();
        let topic_authors = &body.blips_by_topic[&format!("stress-topic-{}", topic_idx)];
        let total_messages: u64 = topic_authors
            .blips
            .values()
            .map(|author| author.len() as u64)
            .sum();

        assert_eq!(total_messages, num_concurrent_writes / num_topics);
    }
}

// Run this test sequentially to avoid "Too many open files" error when combined with other tests
#[tokio::test]
#[serial]
async fn stress_test_concurrent_reads() {
    let (server, _temp_file) = create_test_server().await;
    let server = Arc::new(server);

    // Pre-populate with messages
    let num_topics = 10;
    let messages_per_topic = 500;

    for topic_idx in 0..num_topics {
        for msg_idx in 0..messages_per_topic {
            let topic_id = format!("read-stress-topic-{}", topic_idx);
            let message = format!("Message {} for topic {}", msg_idx, topic_idx);

            server
                .post("/blips/store")
                .json(&create_store_request(
                    &topic_id,
                    message.as_bytes(),
                    msg_idx,
                ))
                .await
                .assert_status(axum::http::StatusCode::CREATED);
        }
    }

    // Now perform concurrent reads
    let num_concurrent_reads = 100;
    let start = Instant::now();
    let mut tasks = Vec::new();

    for i in 0..num_concurrent_reads {
        let server_clone = Arc::clone(&server);
        let topic_id = format!("read-stress-topic-{}", i % num_topics);

        let task = async move {
            let topic_id_clone = topic_id.clone();
            let response = server_clone
                .post("/blips/get")
                .json(&json!({
                    "topics": {
                        &topic_id: {}
                    }
                }))
                .await;

            response.assert_status_ok();
            let body: GetBlipsResponse = response.json();
            let topic_authors = &body.blips_by_topic[&topic_id_clone];
            let total_messages: u64 = topic_authors
                .blips
                .values()
                .map(|author| author.len() as u64)
                .sum();
            assert_eq!(total_messages, messages_per_topic);
        };
        tasks.push(task);
    }

    join_all(tasks).await;

    let duration = start.elapsed();

    println!(
        "Concurrent reads: {} reads in {:?} ({:.2} reads/sec)",
        num_concurrent_reads,
        duration,
        num_concurrent_reads as f64 / duration.as_secs_f64()
    );
}

#[tokio::test]
async fn stress_test_mixed_read_write_operations() {
    let (server, _temp_file) = create_test_server().await;
    let server = Arc::new(server);

    let num_operations = 200;
    let num_topics = 10;

    let start = Instant::now();
    let mut tasks: Vec<Pin<Box<dyn Future<Output = ()>>>> = Vec::new();

    for i in 0..num_operations {
        let server_clone = Arc::clone(&server);
        let topic_id = format!("mixed-topic-{}", i % num_topics);

        if i % 2 == 0 {
            // Write operation
            let task = Box::pin(async move {
                let message = format!("Mixed message {}", i);

                let response = server_clone
                    .post("/blips/store")
                    .json(&create_store_request(&topic_id, message.as_bytes(), i))
                    .await;

                response.assert_status(axum::http::StatusCode::CREATED);
            });
            tasks.push(task);
        } else {
            // Read operation
            let task = Box::pin(async move {
                let response = server_clone
                    .post("/blips/get")
                    .json(&json!({
                        "topics": {
                            topic_id: {}
                        }
                    }))
                    .await;

                response.assert_status_ok();
            });
            tasks.push(task);
        }
    }

    join_all(tasks).await;

    let duration = start.elapsed();

    println!(
        "Mixed operations: {} operations in {:?} ({:.2} ops/sec)",
        num_operations,
        duration,
        num_operations as f64 / duration.as_secs_f64()
    );
}

#[tokio::test]
async fn stress_test_large_messages() {
    let (server, _temp_file) = create_test_server().await;
    let server = Arc::new(server);

    let num_large_messages: usize = 50;
    let message_size: usize = 1024 * 100; // 100KB per message

    let start = Instant::now();
    let mut tasks = Vec::new();

    for i in 0..num_large_messages {
        let server_clone = Arc::clone(&server);
        let topic_id = format!("large-msg-topic-{}", i % 5);

        let task = async move {
            let message = vec![b'X'; message_size];

            let response = server_clone
                .post("/blips/store")
                .json(&create_store_request(&topic_id, &message, i as u64))
                .await;

            response.assert_status(axum::http::StatusCode::CREATED);
        };
        tasks.push(task);
    }

    join_all(tasks).await;

    let duration = start.elapsed();
    let total_data = num_large_messages * message_size;

    println!(
        "Large messages: {} messages ({} MB) in {:?} ({:.2} MB/sec)",
        num_large_messages,
        total_data / (1024 * 1024),
        duration,
        (total_data as f64 / (1024.0 * 1024.0)) / duration.as_secs_f64()
    );

    // Verify retrieval
    for topic_idx in 0..5 {
        let topic_id = format!("large-msg-topic-{}", topic_idx);
        let response = server
            .post("/blips/get")
            .json(&json!({
                "topics": {
                    &topic_id: {}
                }
            }))
            .await;

        response.assert_status_ok();
        let body: GetBlipsResponse = response.json();
        let topic_authors = &body.blips_by_topic[&topic_id];
        let total_messages: u64 = topic_authors
            .blips
            .values()
            .map(|author| author.len() as u64)
            .sum();
        assert_eq!(total_messages, num_large_messages as u64 / 5);
    }
}

#[tokio::test]
async fn stress_test_many_topics() {
    let (server, _temp_file) = create_test_server().await;

    let num_topics = 1000;
    let messages_per_topic = 5;

    let start = Instant::now();

    for topic_idx in 0..num_topics {
        let topic_id = format!("many-topics-{}", topic_idx);
        for msg_idx in 0..messages_per_topic {
            let message = format!("Message {} for topic {}", msg_idx, topic_idx);

            server
                .post("/blips/store")
                .json(&create_store_request(
                    &topic_id,
                    message.as_bytes(),
                    msg_idx,
                ))
                .await
                .assert_status(axum::http::StatusCode::CREATED);
        }
    }

    let store_duration = start.elapsed();

    println!(
        "Many topics: {} topics with {} messages each stored in {:?}",
        num_topics, messages_per_topic, store_duration
    );

    // Test retrieving from multiple topics in one request
    let topic_ids: Vec<TopicId> = (0..100).map(|i| format!("many-topics-{}", i)).collect();

    let mut topics_map = BTreeMap::new();
    for topic_id in &topic_ids {
        topics_map.insert(topic_id.clone(), BTreeMap::<Author, SequenceNumber>::new());
    }

    let start = Instant::now();
    let response = server
        .post("/blips/get")
        .json(&json!({
            "topics": topics_map
        }))
        .await;

    response.assert_status_ok();
    let body: GetBlipsResponse = response.json();

    let retrieve_duration = start.elapsed();

    assert_eq!(body.blips_by_topic.len(), 100);
    for topic_id in &topic_ids {
        let topic_authors = &body.blips_by_topic[topic_id];
        let total_messages: u64 = topic_authors
            .blips
            .values()
            .map(|author| author.len() as u64)
            .sum();
        assert_eq!(total_messages, messages_per_topic);
    }

    println!(
        "Retrieved 100 topics with {} messages each in {:?}",
        messages_per_topic, retrieve_duration
    );
}

#[tokio::test]
async fn stress_test_rapid_sequential_writes() {
    let (server, _temp_file) = create_test_server().await;

    let num_messages = 500;
    let topic_id = "rapid-sequential-topic";

    let start = Instant::now();

    for i in 0..num_messages {
        let message = format!("Rapid message {}", i);

        server
            .post("/blips/store")
            .json(&create_store_request(topic_id, message.as_bytes(), i))
            .await
            .assert_status(axum::http::StatusCode::CREATED);
    }

    let duration = start.elapsed();

    println!(
        "Rapid sequential writes: {} messages in {:?} ({:.2} msg/sec)",
        num_messages,
        duration,
        num_messages as f64 / duration.as_secs_f64()
    );

    // Verify all messages were stored
    let response = server
        .post("/blips/get")
        .json(&json!({
            "topics": {
                topic_id: {}
            }
        }))
        .await;

    response.assert_status_ok();
    let body: GetBlipsResponse = response.json();
    let topic_authors = &body.blips_by_topic[topic_id];
    let total_messages: u64 = topic_authors
        .blips
        .values()
        .map(|author| author.len() as u64)
        .sum();
    assert_eq!(total_messages, num_messages);
}
