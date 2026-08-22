use tauri::{AppHandle, Wry};
use tauri::Manager;
use tauri_plugin_background_service::ServiceManagerHandle;

pub(crate) async fn on_pause(app: AppHandle<Wry>) {
    if !crate::settings::load_settings(&app).background_mode_enabled {
        return;
    }
    let manager = app.state::<ServiceManagerHandle<Wry>>();
    let config = tauri_plugin_background_service::StartConfig {
        service_label: "Dash Chat".into(),
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
}
