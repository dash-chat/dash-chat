use tauri::{AppHandle, Emitter};

use crate::settings;

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
    crate::mailbox::server::set_local_mailbox_server_enabled(&app, enabled)
        .await
        .map_err(|e| e.to_string())?;

    // Emit updated settings so the frontend stays in sync.
    let updated = serde_json::to_value(settings::load_settings(&app))
        .map_err(|e| format!("Failed to serialize settings: {e}"))?;
    let _ = app.emit("settings://updated", updated);

    Ok(())
}
