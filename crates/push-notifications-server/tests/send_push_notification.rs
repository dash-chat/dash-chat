use std::sync::Arc;

use axum::http::StatusCode;
use axum_test::TestServer;
use mockall::predicate::*;

use push_notifications_server::build;
use push_notifications_server::driver::mem::MemDb;
use push_notifications_server::fcm_client::MockFcm;
use push_notifications_server::routes::register_fcm_token::RegisterFcmTokenRequest;
use push_notifications_server::routes::send_push_notification::SendPushNotificationRequest;
use push_notifications_server::types::{FcmToken, PublicKey, PushNotification};

#[tokio::test]
async fn send_push_notification() {
    let public_key = PublicKey::from("test-public-key".to_string());
    let fcm_token = FcmToken::from("test-fcm-token".to_string());
    let notification = PushNotification {
        title: "Hey".to_string(),
        body: "there".to_string(),
    };

    let mut mock_fcm = MockFcm::new();
    mock_fcm
        .expect_validate()
        .once()
        .returning(|| Ok(()));
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

    // 2. Send push notification
    let response = server
        .post("/send-push-notification")
        .json(&SendPushNotificationRequest {
            recipients: vec![public_key],
            notification,
        })
        .await;

    response.assert_status(StatusCode::NO_CONTENT);
}
