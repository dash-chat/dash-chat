use tauri::{AppHandle, State};

use crate::node::AppNodeManager;
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

/// Persist the peer-to-peer switch and rebuild the node in that mode; off, the
/// app syncs through mailboxes only.
#[tauri::command]
pub async fn set_p2p_enabled(
    enabled: bool,
    app: AppHandle,
    app_node_manager: State<'_, AppNodeManager>,
) -> Result<(), String> {
    app_node_manager
        .set_p2p_enabled(&app, enabled)
        .await
        .map_err(|e| e.to_string())
}

#[cfg(not(mobile))]
#[tauri::command]
pub async fn set_local_mailbox_enabled(enabled: bool, app: AppHandle) -> Result<(), String> {
    crate::mailbox::server::set_local_mailbox_server_enabled(&app, enabled)
        .await
        .map_err(|e| e.to_string())
}
