use tauri::{AppHandle, Manager};

pub fn log_webview_version() {
    log::info!(
        "Webview version: {}",
        tauri::webview_version().unwrap_or_else(|e| format!("unknown ({e})")),
    );
}

#[cfg(desktop)]
pub fn log_primary_monitor(handle: &AppHandle) {
    match handle.primary_monitor() {
        Ok(Some(m)) => {
            let size = m.size();
            log::info!(
                "Primary monitor: {}x{} @ scale {} (name {:?})",
                size.width,
                size.height,
                m.scale_factor(),
                m.name(),
            );
        }
        Ok(None) => log::info!("Primary monitor: none detected"),
        Err(err) => log::warn!("Failed to query primary monitor: {err:?}"),
    }
}

pub fn log_system_theme(handle: &AppHandle) {
    let Some(window) = handle.get_webview_window("main") else {
        return;
    };
    match window.theme() {
        Ok(theme) => log::info!("System theme: {theme:?}"),
        Err(err) => log::warn!("Failed to query system theme: {err:?}"),
    }
}

#[tauri::command]
pub fn log_webview_info(user_agent: String) {
    log::info!("Webview user agent: {user_agent}");
}
