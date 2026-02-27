use std::path::PathBuf;
use tauri::{AppHandle, Manager, Runtime};

pub struct FileSystem<R: Runtime>(AppHandle<R>);

const SETTINGS_FILE_NAME: &str = "settings.json";
const LOCAL_MAILBOX_DB_FILE_NAME: &str = "local-mailbox.redb";

impl<R: Runtime> FileSystem<R> {
    pub fn new(handle: &AppHandle<R>) -> Self {
        FileSystem(handle.clone())
    }

    // When DATA_DIR is set, use it directly as the data directory.
    // This is used by mprocs (dev), E2E tests, and any scenario where
    // multiple instances need separate data dirs.
    // Otherwise fall back to the OS local data dir.
    pub fn local_data_dir(&self) -> anyhow::Result<PathBuf> {
        let local_data_path = if let Ok(data_dir) = std::env::var("DATA_DIR") {
            PathBuf::from(data_dir)
        } else {
            self.0.path().local_data_dir()?
        };
        let dashchat_data_path = local_data_path.join("dashchat");
        if !dashchat_data_path.exists() {
            std::fs::create_dir_all(&dashchat_data_path)?;
        }
        Ok(dashchat_data_path)
    }

    pub fn settings_path(&self) -> anyhow::Result<PathBuf> {
        let local_data_dir = self.local_data_dir()?;
        Ok(local_data_dir.join(SETTINGS_FILE_NAME))
    }

    pub fn local_mailbox_db_path(&self) -> anyhow::Result<PathBuf> {
        let local_data_dir = self.local_data_dir()?;
        Ok(local_data_dir.join(LOCAL_MAILBOX_DB_FILE_NAME))
    }
}
