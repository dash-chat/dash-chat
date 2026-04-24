use tauri::AppHandle;

use crate::settings;

#[tauri::command]
pub fn get_settings(app: AppHandle) -> Result<serde_json::Value, String> {
    serde_json::to_value(settings::load_settings(&app))
        .map_err(|err| format!("Failed to serialize settings: {err}"))
}

#[tauri::command]
pub fn set_setting(key: String, value: serde_json::Value, app: AppHandle) -> Result<(), String> {
    settings::set_setting(&app, key, value).map_err(|err| format!("{err:?}"))
}

#[cfg(not(mobile))]
#[tauri::command]
pub async fn set_local_mailbox_enabled(enabled: bool, app: AppHandle) -> Result<(), String> {
    crate::mailbox::server::set_local_mailbox_server_enabled(&app, enabled)
        .await
        .map_err(|e| e.to_string())
}
