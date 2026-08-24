pub(crate) mod build;
mod device_model;
pub mod display;
mod filesystem;
mod network;
mod resources;

use tauri::AppHandle;

pub fn log_device_info(handle: &AppHandle) {
    build::log_build_info(handle);
    log::info!(
        "Operating system: {} {}",
        tauri_plugin_os::type_(),
        tauri_plugin_os::version(),
    );
    device_model::log_device_model(handle);
    display::log_webview_version();
    #[cfg(desktop)]
    display::log_primary_monitor(handle);
    display::log_system_theme(handle);
    log::info!(
        "Locale: {} | Timezone: {}",
        tauri_plugin_os::locale().unwrap_or_else(|| "unknown".to_string()),
        system_timezone(),
    );
    log::info!("Hostname: {}", tauri_plugin_os::hostname());
    filesystem::log_filesystem_paths(handle);
    network::log_network_interfaces();
    resources::spawn_sysinfo_logger();
    network::spawn_interface_change_logger();
}

fn system_timezone() -> String {
    iana_time_zone::get_timezone().unwrap_or_else(|_| "unknown".to_string())
}
