use std::time::Duration;

use fcm_v1::Client;
use fcm_v1::auth::Authenticator;
use fcm_v1::message::Message;
use tokio::sync::OnceCell;

static FCM_CLIENT: OnceCell<Client> = OnceCell::const_new();

pub async fn send_fcm_notification(
    registration_token: &str,
    data: &str,
) -> Result<Message, fcm_v1::Error> {
    let client = FCM_CLIENT
        .get_or_init(|| async move {
            let creds_path = crate::secret::GOOGLE_APPLICATION_CREDENTIALS;
            let project_id = crate::secret::FCM_PROJECT_ID;
            let authenticator = Authenticator::service_account_from_file(creds_path)
                .await
                .unwrap();
            Client::new(authenticator, project_id, true, Duration::from_secs(10))
        })
        .await;

    let mut test_noti = fcm_v1::message::Notification::default();
    test_noti.title = None;
    test_noti.body = Some(data.to_owned());

    let mut test_message = Message::default();
    test_message.notification = Some(test_noti);
    test_message.token = Some(registration_token.to_owned());

    client.send(&test_message).await
}
