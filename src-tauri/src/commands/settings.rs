use tauri::AppHandle;

use crate::settings;

#[tauri::command]
pub fn get_settings(app: AppHandle) -> serde_json::Value {
    serde_json::to_value(settings::load_settings(&app)).unwrap_or_default()
}

#[tauri::command]
pub fn set_setting(key: String, value: serde_json::Value, app: AppHandle) -> Result<(), String> {
    let mut current = serde_json::to_value(settings::load_settings(&app)).unwrap_or_default();

    if let Some(obj) = current.as_object_mut() {
        obj.insert(key.clone(), value);
    }

    let settings = serde_json::from_value::<settings::Settings>(current)
        .map_err(|err| format!("Invalid setting {key}: {err}"))?;

    settings::save_settings(&app, &settings);
    Ok(())
}
