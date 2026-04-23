use tauri::AppHandle;

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

    Ok(())
}

#[cfg(mobile)]
#[tauri::command]
pub async fn set_notifications_enabled(enabled: bool, app: AppHandle) -> Result<(), String> {
    let mut s = settings::load_settings(&app);
    s.notifications_enabled = enabled;
    settings::save_settings(&app, &s);

    if enabled {
        crate::push_notifications::register_push_notifications(&app)
            .await
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[cfg(not(mobile))]
#[tauri::command]
pub async fn set_local_mailbox_enabled(enabled: bool, app: AppHandle) -> Result<(), String> {
    crate::mailbox::server::set_local_mailbox_server_enabled(&app, enabled)
        .await
        .map_err(|e| e.to_string())
}
