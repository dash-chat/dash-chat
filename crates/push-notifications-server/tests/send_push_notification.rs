use std::sync::Arc;

use axum::http::StatusCode;
use axum_test::TestServer;
use mockall::predicate::*;

use push_notifications_client::requests::{
    AddTopicSubscriptionsRequest, NotifyTopicsRequest, RegisterFcmTokenRequest,
    RemoveTopicSubscriptionsRequest, UpdateTopicSubscriptionsRequest,
};
use push_notifications_client::types::{FcmToken, OperationId, PublicKey, TopicId};
use push_notifications_server::build;
use push_notifications_server::driver::Driver;
use push_notifications_server::driver::mem::MemDb;
use push_notifications_server::fcm_client::{MockFcm, SendResult};

#[tokio::test]
async fn notify_topic_sends_to_subscribers() {
    let public_key = PublicKey::from("aa".repeat(32));
    let fcm_token = FcmToken::from("test-fcm-token".to_string());
    let topic_id = TopicId::from("bb".repeat(32));
    let op_id = OperationId::from("test-op".to_string());

    let mut mock_fcm = MockFcm::new();
    mock_fcm.expect_validate().once().returning(|| Ok(()));
    mock_fcm
        .expect_send_push_notification()
        .once()
        .withf(|token, notif| token == "test-fcm-token" && notif.body == "test-op")
        .returning(|_, _| SendResult::Ok);

    let app = build(Arc::new(MemDb::new()), Arc::new(mock_fcm))
        .await
        .unwrap();

    let server = TestServer::new(app).unwrap();

    // 1. Register FCM token
    let response = server
        .post("/register-fcm-token")
        .json(&RegisterFcmTokenRequest {
            public_key: public_key.clone(),
            fcm_token: fcm_token.clone(),
        })
        .await;
    response.assert_status(StatusCode::NO_CONTENT);

    // 2. Subscribe to topic
    let response = server
        .post("/topic-subscriptions/add")
        .json(&AddTopicSubscriptionsRequest {
            public_key: public_key.clone(),
            topic_ids: [topic_id.clone()].into(),
        })
        .await;
    response.assert_status(StatusCode::NO_CONTENT);

    // 3. Notify topic — should send push to the subscriber
    let response = server
        .post("/notify-topic")
        .json(&NotifyTopicsRequest {
            topics_to_notify: [(topic_id, [op_id].into())].into(),
        })
        .await;
    response.assert_status(StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn notify_topic_skips_unsubscribed() {
    let public_key = PublicKey::from("aa".repeat(32));
    let fcm_token = FcmToken::from("test-fcm-token".to_string());
    let topic_id = TopicId::from("bb".repeat(32));

    let mut mock_fcm = MockFcm::new();
    mock_fcm.expect_validate().once().returning(|| Ok(()));
    // No send_push_notification calls expected

    let app = build(Arc::new(MemDb::new()), Arc::new(mock_fcm))
        .await
        .unwrap();

    let server = TestServer::new(app).unwrap();

    // 1. Register FCM token but don't subscribe
    let response = server
        .post("/register-fcm-token")
        .json(&RegisterFcmTokenRequest {
            public_key: public_key.clone(),
            fcm_token: fcm_token.clone(),
        })
        .await;
    response.assert_status(StatusCode::NO_CONTENT);

    // 2. Notify topic — no subscribers, should not send
    let response = server
        .post("/notify-topic")
        .json(&NotifyTopicsRequest {
            topics_to_notify: [(topic_id, [OperationId::from("test-op".to_string())].into())]
                .into(),
        })
        .await;
    response.assert_status(StatusCode::NO_CONTENT);
}

// --- unsubscribe ---

#[tokio::test]
async fn unsubscribe_prevents_notification() {
    let public_key = PublicKey::from("aa".repeat(32));
    let fcm_token = FcmToken::from("test-fcm-token".to_string());
    let topic_id = TopicId::from("bb".repeat(32));

    let mut mock_fcm = MockFcm::new();
    mock_fcm.expect_validate().once().returning(|| Ok(()));
    // No send expected after unsubscribe

    let app = build(Arc::new(MemDb::new()), Arc::new(mock_fcm))
        .await
        .unwrap();
    let server = TestServer::new(app).unwrap();

    // Register + subscribe
    server
        .post("/register-fcm-token")
        .json(&RegisterFcmTokenRequest {
            public_key: public_key.clone(),
            fcm_token,
        })
        .await
        .assert_status(StatusCode::NO_CONTENT);

    server
        .post("/topic-subscriptions/add")
        .json(&AddTopicSubscriptionsRequest {
            public_key: public_key.clone(),
            topic_ids: [topic_id.clone()].into(),
        })
        .await
        .assert_status(StatusCode::NO_CONTENT);

    // Unsubscribe
    server
        .post("/topic-subscriptions/remove")
        .json(&RemoveTopicSubscriptionsRequest {
            public_key: public_key.clone(),
            topic_ids: [topic_id.clone()].into(),
        })
        .await
        .assert_status(StatusCode::NO_CONTENT);

    // Notify — should NOT send (unsubscribed)
    server
        .post("/notify-topic")
        .json(&NotifyTopicsRequest {
            topics_to_notify: [(topic_id, [OperationId::from("op-1".to_string())].into())].into(),
        })
        .await
        .assert_status(StatusCode::NO_CONTENT);
}

// --- set_subscriptions ---

#[tokio::test]
async fn set_subscriptions_replaces_and_notifies_correctly() {
    let public_key = PublicKey::from("aa".repeat(32));
    let fcm_token = FcmToken::from("test-fcm-token".to_string());
    let topic_a = TopicId::from("cc".repeat(32));
    let topic_b = TopicId::from("dd".repeat(32));

    let mut mock_fcm = MockFcm::new();
    mock_fcm.expect_validate().once().returning(|| Ok(()));
    // Only topic-b notification should fire (topic-a was replaced away)
    mock_fcm
        .expect_send_push_notification()
        .once()
        .withf(|token, notif| token == "test-fcm-token" && notif.title == "dd".repeat(32))
        .returning(|_, _| SendResult::Ok);

    let app = build(Arc::new(MemDb::new()), Arc::new(mock_fcm))
        .await
        .unwrap();
    let server = TestServer::new(app).unwrap();

    // Register token
    server
        .post("/register-fcm-token")
        .json(&RegisterFcmTokenRequest {
            public_key: public_key.clone(),
            fcm_token,
        })
        .await
        .assert_status(StatusCode::NO_CONTENT);

    // Subscribe to topic-a
    server
        .post("/topic-subscriptions/add")
        .json(&AddTopicSubscriptionsRequest {
            public_key: public_key.clone(),
            topic_ids: [topic_a.clone()].into(),
        })
        .await
        .assert_status(StatusCode::NO_CONTENT);

    // Replace subscriptions: drop topic-a, add topic-b
    server
        .post("/topic-subscriptions/update")
        .json(&UpdateTopicSubscriptionsRequest {
            public_key: public_key.clone(),
            topic_ids: [topic_b.clone()].into(),
        })
        .await
        .assert_status(StatusCode::NO_CONTENT);

    // Notify topic-a — should NOT send (replaced away)
    server
        .post("/notify-topic")
        .json(&NotifyTopicsRequest {
            topics_to_notify: [(topic_a, [OperationId::from("op-1".to_string())].into())].into(),
        })
        .await
        .assert_status(StatusCode::NO_CONTENT);

    // Notify topic-b — should send
    server
        .post("/notify-topic")
        .json(&NotifyTopicsRequest {
            topics_to_notify: [(topic_b, [OperationId::from("op-2".to_string())].into())].into(),
        })
        .await
        .assert_status(StatusCode::NO_CONTENT);
}

// --- error scenarios ---

#[tokio::test]
async fn fcm_transient_failure_does_not_remove_token() {
    let public_key = PublicKey::from("aa".repeat(32));
    let fcm_token = FcmToken::from("test-fcm-token".to_string());
    let topic_id = TopicId::from("bb".repeat(32));

    let mut mock_fcm = MockFcm::new();
    mock_fcm.expect_validate().once().returning(|| Ok(()));
    mock_fcm
        .expect_send_push_notification()
        .once()
        .returning(|_, _| SendResult::Error("FCM service unavailable".to_string()));

    let db = Arc::new(MemDb::new());

    let app = build(db.clone(), Arc::new(mock_fcm)).await.unwrap();
    let server = TestServer::new(app).unwrap();

    server
        .post("/register-fcm-token")
        .json(&RegisterFcmTokenRequest {
            public_key: public_key.clone(),
            fcm_token: fcm_token.clone(),
        })
        .await
        .assert_status(StatusCode::NO_CONTENT);

    server
        .post("/topic-subscriptions/add")
        .json(&AddTopicSubscriptionsRequest {
            public_key: public_key.clone(),
            topic_ids: [topic_id.clone()].into(),
        })
        .await
        .assert_status(StatusCode::NO_CONTENT);

    // Notify — FCM fails but the endpoint should still return 204
    server
        .post("/notify-topic")
        .json(&NotifyTopicsRequest {
            topics_to_notify: [(topic_id, [OperationId::from("op-1".to_string())].into())].into(),
        })
        .await
        .assert_status(StatusCode::NO_CONTENT);

    // Token should be preserved (transient error, not invalid token)
    let tokens = db.get_fcm_tokens(&[public_key.clone()]).await.unwrap();
    assert_eq!(tokens.get(&public_key), Some(&fcm_token));
}

#[tokio::test]
async fn notify_subscriber_without_token_does_not_fail() {
    let public_key = PublicKey::from("ee".repeat(32));
    let topic_id = TopicId::from("bb".repeat(32));

    let mut mock_fcm = MockFcm::new();
    mock_fcm.expect_validate().once().returning(|| Ok(()));
    // No send expected — user has no FCM token registered

    let db = Arc::new(MemDb::new());

    // Subscribe directly via the driver (no token registered)
    db.add_topic_subscriptions(&public_key, &[topic_id.clone()].into())
        .await
        .unwrap();

    let app = build(db, Arc::new(mock_fcm)).await.unwrap();
    let server = TestServer::new(app).unwrap();

    // Notify — should gracefully skip the subscriber with no token
    server
        .post("/notify-topic")
        .json(&NotifyTopicsRequest {
            topics_to_notify: [(topic_id, [OperationId::from("op-1".to_string())].into())].into(),
        })
        .await
        .assert_status(StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn invalid_token_is_removed() {
    let public_key = PublicKey::from("aa".repeat(32));
    let fcm_token = FcmToken::from("expired-token".to_string());
    let topic_id = TopicId::from("bb".repeat(32));

    let mut mock_fcm = MockFcm::new();
    mock_fcm.expect_validate().once().returning(|| Ok(()));
    mock_fcm
        .expect_send_push_notification()
        .with(eq("expired-token"), always())
        .once()
        .returning(|_, _| SendResult::InvalidToken);

    let db = Arc::new(MemDb::new());

    db.store_fcm_token(&public_key, &fcm_token).await.unwrap();
    db.add_topic_subscriptions(&public_key, &[topic_id.clone()].into())
        .await
        .unwrap();

    let app = build(db.clone(), Arc::new(mock_fcm)).await.unwrap();
    let server = TestServer::new(app).unwrap();

    // Notify — FCM reports token invalid, should remove it
    server
        .post("/notify-topic")
        .json(&NotifyTopicsRequest {
            topics_to_notify: [(topic_id, [OperationId::from("op-1".to_string())].into())].into(),
        })
        .await
        .assert_status(StatusCode::NO_CONTENT);

    // Token should have been removed from the database
    let tokens = db.get_fcm_tokens(&[public_key.clone()]).await.unwrap();
    assert_eq!(tokens.get(&public_key), None);
}
