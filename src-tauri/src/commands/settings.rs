use tauri::AppHandle;

use crate::settings;

#[tauri::command]
pub fn get_qr_color(app: AppHandle) -> Option<String> {
    settings::load_qr_color(&app)
}

#[tauri::command]
pub fn set_qr_color(color: String, app: AppHandle) {
    settings::save_qr_color(&app, color);
}
