use std::path::PathBuf;
use std::sync::OnceLock;
use tauri::{AppHandle, Manager, Runtime};

/// Hold the lock file handle for the lifetime of the process so the exclusive
/// lock is never released while the app is running.
static DATA_DIR_LOCK: OnceLock<std::fs::File> = OnceLock::new();

/// In dev mode, if DATA_DIR is not already set, auto-select the first available
/// `.dbs/dev/agent-N` directory (using an exclusive file lock to detect running instances).
///
/// Then, if DATA_DIR is set (either externally or by auto-detection), isolate
/// XDG directories to prevent WebKitGTK SQLite lock conflicts between instances.
pub fn init_data_dir() {
    if std::env::var("DATA_DIR").is_err() && tauri::is_dev() && cfg!(not(mobile)) {
        let project_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
        let base = project_root.join(".dbs/dev");

        let mut found = false;
        for n in 1..=100 {
            let dir = base.join(format!("agent-{n}"));
            std::fs::create_dir_all(&dir).ok();
            let lock_path = dir.join(".lock");

            if let Ok(file) = std::fs::File::create(&lock_path) {
                use fs2::FileExt;
                if file.try_lock_exclusive().is_ok() {
                    std::env::set_var("DATA_DIR", &dir);
                    let _ = DATA_DIR_LOCK.set(file);
                    found = true;
                    break;
                }
            }
        }
        if !found {
            eprintln!(
                "WARNING: could not find an available agent slot in {}",
                base.display()
            );
        }
    }

    // Isolate XDG/WebKitGTK data directories per instance.
    if let Ok(data_dir) = std::env::var("DATA_DIR") {
        let data_dir = std::path::Path::new(&data_dir);
        std::env::set_var("XDG_DATA_HOME", data_dir.join(".local/share"));
        std::env::set_var("XDG_CACHE_HOME", data_dir.join(".cache"));
        std::env::set_var("XDG_CONFIG_HOME", data_dir.join(".config"));
    }
}

pub struct FileSystem<R: Runtime>(AppHandle<R>);

const SETTINGS_FILE_NAME: &str = "settings.json";
const LOCAL_MAILBOX_DB_FILE_NAME: &str = "local-mailbox.redb";
const DASHCHAT_DATA_FOLDER: &str = "studio.darksoil.dashchat";

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
        let dashchat_data_path = local_data_path.join(DASHCHAT_DATA_FOLDER);
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
