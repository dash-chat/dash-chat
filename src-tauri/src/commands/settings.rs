use tauri::{AppHandle, Emitter};

use crate::settings;

#[cfg(not(mobile))]
use crate::mailbox;

#[tauri::command]
pub fn get_settings(app: AppHandle) -> Result<serde_json::Value, String> {
    serde_json::to_value(settings::load_settings(&app))
        .map_err(|err| format!("Failed to serialize settings: {err}"))
}

#[tauri::command]
pub fn set_setting(key: String, value: serde_json::Value, app: AppHandle) -> Result<(), String> {
    let mut current = serde_json::to_value(settings::load_settings(&app)).unwrap_or_default();

    let known_keys = current
        .as_object()
        .map(|obj| obj.keys().cloned().collect::<Vec<_>>())
        .unwrap_or_default();

    if !known_keys.contains(&key) {
        return Err(format!("Unknown setting: {key}"));
    }

    if let Some(obj) = current.as_object_mut() {
        obj.insert(key.clone(), value);
    }

    let settings = serde_json::from_value::<settings::Settings>(current)
        .map_err(|err| format!("Invalid setting {key}: {err}"))?;

    settings::save_settings(&app, &settings);

    let updated = serde_json::to_value(&settings)
        .map_err(|err| format!("Failed to serialize settings: {err}"))?;
    let _ = app.emit("settings://updated", updated);

    Ok(())
}

#[cfg(not(mobile))]
#[tauri::command]
pub async fn set_local_mailbox_enabled(enabled: bool, app: AppHandle) -> Result<(), String> {
    settings::save_mailbox_enabled(&app, enabled);

    // The autostart plugin is only registered in release builds.
    if !tauri::is_dev() {
        use tauri_plugin_autostart::ManagerExt;
        let autostart = app.autolaunch();
        if enabled {
            autostart.enable().map_err(|e| e.to_string())?;
        } else {
            autostart.disable().map_err(|e| e.to_string())?;
        }
    }

    let r = if enabled {
        mailbox::server::start_local_mailbox(&app).await
    } else {
        mailbox::server::stop_local_mailbox(&app).await
    };
    r.map_err(|e| e.to_string())?;

    // Emit updated settings so the frontend stays in sync.
    let updated = serde_json::to_value(settings::load_settings(&app))
        .map_err(|e| format!("Failed to serialize settings: {e}"))?;
    let _ = app.emit("settings://updated", updated);

    Ok(())
}
