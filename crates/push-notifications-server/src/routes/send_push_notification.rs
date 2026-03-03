use axum::{Json, extract::State, http::StatusCode};
use futures::future::join_all;
use serde::Deserialize;

use crate::{
    AppState,
    error::AppError,
    types::{PublicKey, PushNotification},
};

#[derive(Deserialize, serde::Serialize)]
pub struct SendPushNotificationRequest {
    pub recipients: Vec<PublicKey>,
    pub notification: PushNotification,
}

pub(crate) async fn send_push_notification(
    State(state): State<AppState>,
    Json(req): Json<SendPushNotificationRequest>,
) -> Result<StatusCode, AppError> {
    let tasks = req.recipients.iter().map(|public_key| {
        let state = state.clone();
        let public_key = public_key.clone();
        let notification = req.notification.clone();
        async move {
            match state.db.get_fcm_token(&public_key).await {
                Ok(Some(fcm_token)) => {
                    if let Err(e) = state.fcm.send_push_notification(&fcm_token, &notification).await {
                        tracing::warn!(public_key = %public_key, "failed to send FCM notification: {e:#}");
                    } else {
                        tracing::info!(public_key = %public_key, "sent push notification");
                    }
                }
                Ok(None) => {
                    tracing::debug!(public_key = %public_key, "no FCM token registered, skipping");
                }
                Err(e) => {
                    tracing::warn!(public_key = %public_key, "failed to look up FCM token: {e:#}");
                }
            }
        }
    });

    join_all(tasks).await;

    Ok(StatusCode::NO_CONTENT)
}
