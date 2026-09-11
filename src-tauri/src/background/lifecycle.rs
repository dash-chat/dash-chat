use tauri::Manager;
use tauri::{AppHandle, Wry};
use tauri_plugin_background_service::ServiceManagerHandle;

pub(crate) async fn on_pause(app: AppHandle<Wry>) {
    if !crate::settings::load_settings(&app).background_mode_enabled {
        return;
    }
    let manager = app.state::<ServiceManagerHandle<Wry>>();
    let config = tauri_plugin_background_service::StartConfig {
        service_label: sonix_i18n::t!("backgroundServiceRunning"),
        foreground_service_type: "remoteMessaging".into(),
    };
    if let Err(e) = manager.start(app.clone(), config).await {
        log::error!("[android-lifecycle] startService failed: {e:?}");
    }
}

pub(crate) async fn on_resume(app: AppHandle<Wry>) {
    let manager = app.state::<ServiceManagerHandle<Wry>>();
    if manager.is_running().await {
        if let Err(e) = manager.stop().await {
            log::error!("[android-lifecycle] stopService failed: {e:?}");
        }
    }
    wakeup_cloud_mailbox(&app).await;
}

/// Force-poll the cloud mailbox on foreground.
///
/// Android denies network access to backgrounded apps, so the polls that ran
/// while we were away failed and left the cloud mailbox backed off with a high
/// consecutive-error count — which the UI renders as disconnected until the next
/// scheduled poll, up to `stopped_interval` later. Waking it clears the backoff
/// and re-measures immediately, so what the user sees on resume reflects the
/// connection they have now rather than the one they didn't have in the
/// background.
async fn wakeup_cloud_mailbox(app: &AppHandle<Wry>) {
    let Some(app_node_manager) = app.try_state::<crate::node::AppNodeManager>() else {
        return;
    };
    let Ok(node) = app_node_manager.get().await else {
        return;
    };
    match crate::mailbox::cloud_mailbox_id(&node).await {
        Some(cloud_id) => node.mailboxes.wakeup(cloud_id),
        None => node.mailboxes.trigger_sync(),
    }
}
