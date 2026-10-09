#[cfg(mobile)]
mod receive_push_notification;
#[cfg(mobile)]
mod register_push_notifications_token;
mod subscribe_to_push_notifications_for_topics;

#[cfg(mobile)]
pub(crate) use register_push_notifications_token::RegisterPushNotificationsTokenTask;
#[cfg(mobile)]
pub(crate) use subscribe_to_push_notifications_for_topics::SubscribeToPushNotificationsForTopicsTask;

#[cfg(target_os = "ios")]
pub mod nse_signal;

#[cfg(target_os = "android")]
mod android;

const NOTIFICATIONS_ENABLED_UPDATED_EVENT: &str = "settings://updated-notifications_enabled";
#[cfg(mobile)]
const NEW_FCM_TOKEN_EVENT: &str = "notification://new-fcm-token";

#[cfg(mobile)]
const PRODUCTION_PUSH_NOTIFICATIONS_SERVER_URL: &str =
    "https://push-notifications.production.darksoil.studio";

/// Returns the push notifications server URL to use.
///
/// Resolution order:
/// 1. `PUSH_NOTIFICATIONS_SERVER_URL` runtime env var (E2E tests)
/// 2. `PUSH_NOTIFICATIONS_SERVER_URL` compile-time env var (set by build.rs in debug builds)
/// 3. Production URL
#[cfg(mobile)]
pub(crate) fn push_notifications_url() -> String {
    if let Ok(url) = std::env::var("PUSH_NOTIFICATIONS_SERVER_URL") {
        if !(url.starts_with("http://") || url.starts_with("https://")) {
            log::error!(
                "PUSH_NOTIFICATIONS_SERVER_URL env var is not a valid URL: {url}, falling back to next option"
            );
        } else {
            return url;
        }
    }
    if let Some(url) = option_env!("PUSH_NOTIFICATIONS_SERVER_URL") {
        log::info!("Using compile-time PUSH_NOTIFICATIONS_SERVER_URL: {url}");
        return url.to_string();
    }
    PRODUCTION_PUSH_NOTIFICATIONS_SERVER_URL.to_string()
}
