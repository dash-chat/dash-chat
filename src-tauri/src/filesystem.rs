use std::path::PathBuf;
use tauri::{AppHandle, Manager, Runtime};

pub struct FileSystem<R: Runtime>(AppHandle<R>);

const SETTINGS_FILE_NAME: &str = "settings.json";
const LOCAL_MAILBOX_DB_FILE_NAME: &str = "local-mailbox.redb";

impl<R: Runtime> FileSystem<R> {
    pub fn new(handle: &AppHandle<R>) -> Self {
        FileSystem(handle.clone())
    }

    // In production, use the local data dir from the operating system.
    // In desktop development, use DEV_DBS_PATH/agent-{AGENT} (set in mprocs.yaml) so multiple
    // agents can run side-by-side. On mobile development we fall through to the OS
    // data dir because DEV_DBS_PATH points to the build machine, not the device.
    pub fn local_data_dir(&self) -> anyhow::Result<PathBuf> {
        let local_data_path = if cfg!(mobile) || !tauri::is_dev() {
            self.0.path().local_data_dir()?
        } else {
            let base = PathBuf::from(std::env::var("DEV_DBS_PATH")?);
            base.join(format!("agent-{}", std::env::var("AGENT")?))
        };
        if !local_data_path.exists() {
            std::fs::create_dir_all(&local_data_path)?;
        }
        Ok(local_data_path)
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
