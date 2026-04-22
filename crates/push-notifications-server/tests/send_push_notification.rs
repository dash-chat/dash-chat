use std::sync::Arc;

use axum::http::StatusCode;
use axum_test::TestServer;
use mockall::predicate::*;

use push_notifications_client::requests::{
    NotifyTopicsRequest, RegisterFcmTokenRequest, SubscribeRequest,
};
use push_notifications_client::types::{FcmToken, OperationId, PublicKey, TopicId};
use push_notifications_server::build;
use push_notifications_server::driver::mem::MemDb;
use push_notifications_server::fcm_client::MockFcm;

#[tokio::test]
async fn notify_topic_sends_to_subscribers() {
    let public_key = PublicKey::from("test-public-key".to_string());
    let fcm_token = FcmToken::from("test-fcm-token".to_string());
    let topic_id = TopicId::from("test-topic".to_string());
    let op_id = OperationId::from("test-op".to_string());

    let mut mock_fcm = MockFcm::new();
    mock_fcm.expect_validate().once().returning(|| Ok(()));
    mock_fcm
        .expect_send_push_notification()
        .with(eq("test-fcm-token"), always())
        .once()
        .returning(|_, _| Ok(()));

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
        .post("/subscribe")
        .json(&SubscribeRequest {
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
    let public_key = PublicKey::from("test-public-key".to_string());
    let fcm_token = FcmToken::from("test-fcm-token".to_string());
    let topic_id = TopicId::from("test-topic".to_string());

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
