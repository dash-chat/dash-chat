use jni::objects::JClass;
use jni::JNIEnv;
use tauri_plugin_notification::{NotificationData, ReceivePushNotificationContext};

/// The notification Android shows while the app handles a push in the
/// background: the plugin runs the push handler under a foreground service,
/// and Android requires every foreground service to show one. Only the title
/// is shown. Asked before `receive_push_notification` runs, so with no app
/// process up it brings up i18n itself.
#[tauri_plugin_notification::fetching_push_notification_data]
pub fn fetching_push_notification_data(
    _context: ReceivePushNotificationContext,
) -> NotificationData {
    if crate::APP_HANDLE.get().is_none() {
        crate::i18n::init_i18n();
    }
    NotificationData {
        title: Some(sonix_i18n::t!("checkingForMessages")),
        ..Default::default()
    }
}
