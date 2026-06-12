use tauri::AppHandle;

pub fn log_filesystem_paths(handle: &AppHandle) {
    let fs = match crate::filesystem::FileSystem::new(handle) {
        Ok(fs) => fs,
        Err(err) => {
            log::warn!("Failed to resolve filesystem paths: {err:?}");
            return;
        }
    };
    log::info!("App root dir: {}", fs.app_root_dir().display());
    log::info!("App data dir: {}", fs.app_data_dir().display());
    log::info!("Logs dir: {}", fs.logs_dir().display());
    log::info!("Settings path: {}", fs.settings_path().display());
    #[cfg(desktop)]
    log::info!("Local mailbox db path: {}", fs.local_mailbox_db_path().display(),);
}
