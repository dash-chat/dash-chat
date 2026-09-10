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
    probe_cloud_mailbox(&app).await;
    rearm_mdns_discovery(&app).await;
}

/// A hub that announced while we were backgrounded was missed, and the browse
/// re-query that would find it can be up to an hour away.
async fn rearm_mdns_discovery(app: &AppHandle<Wry>) {
    let Some(app_node_manager) = app.try_state::<crate::node::AppNodeManager>() else {
        return;
    };
    app_node_manager.rearm_mdns_discovery().await;
}

/// Poll the cloud mailbox on foreground.
///
/// Android denies network access to backgrounded apps, so the polls that ran
/// while we were away failed and left the cloud mailbox backed off, with the
/// next scheduled poll up to `stopped_interval` away. Probing re-measures
/// immediately without presuming the result: one success restores Active, and
/// the UI already discounts failures recorded before it resumed rendering.
async fn probe_cloud_mailbox(app: &AppHandle<Wry>) {
    let Some(app_node_manager) = app.try_state::<crate::node::AppNodeManager>() else {
        return;
    };
    let Ok(node) = app_node_manager.get().await else {
        return;
    };
    match crate::mailbox::cloud_mailbox_id(&node).await {
        Some(cloud_id) => node.mailboxes.probe(cloud_id).await,
        None => node.mailboxes.trigger_poll_loop(),
    }
}
