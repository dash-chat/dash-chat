use std::sync::Arc;

use axum::http::StatusCode;
use axum_test::{TestServer, TestServerConfig, Transport};
use mailbox_server::test_utils::create_test_db;
use push_notifications_client::client::PushNotificationsClient;
use push_notifications_client::requests::{AddTopicSubscriptionsRequest, RegisterFcmTokenRequest};
use push_notifications_client::types::{FcmToken, TopicId, VerifyingKey};
use push_notifications_server::driver::mem::MemDb;
use push_notifications_server::fcm_client::{MockFcm, SendResult};
use serde_json::json;

/// Starts a push notifications server with the given mock FCM on a random port.
/// Returns the base URL (e.g. "http://127.0.0.1:12345").
async fn start_push_server(mock_fcm: MockFcm) -> String {
    let push_app = push_notifications_server::build(Arc::new(MemDb::new()), Arc::new(mock_fcm))
        .await
        .unwrap();

    let push_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let push_addr = push_listener.local_addr().unwrap();
    let push_url = format!("http://{push_addr}");

    tokio::spawn(async move {
        axum::serve(push_listener, push_app).await.unwrap();
    });

    push_url
}

/// Creates a mailbox TestServer connected to the given push notifications URL.
/// Returns the push_tasks handle so tests can await completion instead of sleeping.
fn start_mailbox_server(
    push_url: String,
) -> (
    TestServer,
    tempfile::NamedTempFile,
    Arc<tokio::sync::Mutex<tokio::task::JoinSet<()>>>,
) {
    let (db, temp_file) = create_test_db();
    let push_client = PushNotificationsClient::new(push_url).unwrap();
    let push_tasks = Arc::new(tokio::sync::Mutex::new(tokio::task::JoinSet::new()));
    let app = mailbox_server::create_app(Arc::new(db), Some(Arc::new(push_client)), push_tasks.clone());
    let config = TestServerConfig {
        transport: Some(Transport::HttpRandomPort),
        ..TestServerConfig::default()
    };
    let server = TestServer::new_with_config(app, config).unwrap();
    (server, temp_file, push_tasks)
}

/// Waits for all spawned push notification tasks to complete, with a timeout.
async fn drain_push_tasks(push_tasks: &Arc<tokio::sync::Mutex<tokio::task::JoinSet<()>>>) {
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let done = {
            let mut tasks = push_tasks.lock().await;
            // try_join_next returns None when there are no remaining tasks
            while tasks.try_join_next().is_some() {}
            tasks.is_empty()
        };
        if done {
            break;
        }
        if tokio::time::Instant::now() >= deadline {
            panic!("push tasks did not complete within 5s");
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
}

/// Full integration test: subscribe → store blobs → push notification sent.
#[tokio::test]
async fn mailbox_store_triggers_push_notification() {
    let verifying_key =
        VerifyingKey::from("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string());
    let fcm_token = FcmToken::from("fcm-token-xyz".to_string());
    let topic_id = TopicId::from("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string());

    let mut mock_fcm = MockFcm::new();
    mock_fcm.expect_validate().once().returning(|| Ok(()));
    mock_fcm
        .expect_send_push_notification()
        .withf(|token, _| token == "fcm-token-xyz")
        .once()
        .returning(|_, _| SendResult::Ok);

    let push_url = start_push_server(mock_fcm).await;

    // Register FCM token + subscribe to topic
    let http = reqwest::Client::new();

    let resp = http
        .post(format!("{push_url}/fcm-tokens/register"))
        .json(&RegisterFcmTokenRequest {
            verifying_key: verifying_key.clone(),
            fcm_token,
        })
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);

    let resp = http
        .post(format!("{push_url}/topic-subscriptions/add"))
        .json(&AddTopicSubscriptionsRequest {
            verifying_key,
            topic_ids: [topic_id].into(),
        })
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);

    // Mailbox server connected to the push server
    let (mailbox, _tmp, push_tasks) = start_mailbox_server(push_url);

    let blob_data = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"encrypted-message-payload");

    let resp = mailbox
        .post("/blobs/store")
        .json(&json!({
            "blobs": {
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb": {
                    "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee": {
                        "0": blob_data
                    }
                }
            }
        }))
        .await;
    resp.assert_status(StatusCode::CREATED);

    drain_push_tasks(&push_tasks).await;

    // MockFcm panics on drop if expect_send_push_notification wasn't called exactly once
}

/// Storing a blob for a topic with no subscribers should not trigger any FCM call.
#[tokio::test]
async fn mailbox_store_no_subscribers_no_push() {
    let mut mock_fcm = MockFcm::new();
    mock_fcm.expect_validate().once().returning(|| Ok(()));
    // No send expected

    let push_url = start_push_server(mock_fcm).await;
    let (mailbox, _tmp, push_tasks) = start_mailbox_server(push_url);

    let blob_data = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"some-data");

    let resp = mailbox
        .post("/blobs/store")
        .json(&json!({
            "blobs": {
                "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd": {
                    "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee": {
                        "0": blob_data
                    }
                }
            }
        }))
        .await;
    resp.assert_status(StatusCode::CREATED);

    drain_push_tasks(&push_tasks).await;
    // MockFcm panics on drop if send_push_notification was called unexpectedly
}

/// Storing the same blob twice should only trigger a push notification the first time
/// (watermark doesn't advance on duplicate data).
#[tokio::test]
async fn mailbox_store_duplicate_blob_no_second_push() {
    let verifying_key =
        VerifyingKey::from("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string());
    let fcm_token = FcmToken::from("fcm-token-xyz".to_string());
    let topic_id = TopicId::from("cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc".to_string());

    let mut mock_fcm = MockFcm::new();
    mock_fcm.expect_validate().once().returning(|| Ok(()));
    // Expect exactly ONE push, not two
    mock_fcm
        .expect_send_push_notification()
        .withf(|token, _| token == "fcm-token-xyz")
        .once()
        .returning(|_, _| SendResult::Ok);

    let push_url = start_push_server(mock_fcm).await;

    let http = reqwest::Client::new();

    http.post(format!("{push_url}/fcm-tokens/register"))
        .json(&RegisterFcmTokenRequest {
            verifying_key: verifying_key.clone(),
            fcm_token,
        })
        .send()
        .await
        .unwrap();

    http.post(format!("{push_url}/topic-subscriptions/add"))
        .json(&AddTopicSubscriptionsRequest {
            verifying_key,
            topic_ids: [topic_id].into(),
        })
        .send()
        .await
        .unwrap();

    let (mailbox, _tmp, push_tasks) = start_mailbox_server(push_url);

    let blob_data = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"message-content");

    let store_body = json!({
        "blobs": {
            "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc": {
                "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee": {
                    "0": blob_data
                }
            }
        }
    });

    // First store — should trigger push
    let resp = mailbox.post("/blobs/store").json(&store_body).await;
    resp.assert_status(StatusCode::CREATED);
    drain_push_tasks(&push_tasks).await;

    // Second store with same data — watermark won't advance, no push
    let resp = mailbox.post("/blobs/store").json(&store_body).await;
    resp.assert_status(StatusCode::CREATED);
    drain_push_tasks(&push_tasks).await;

    // MockFcm asserts exactly 1 call on drop
}
