use std::path::PathBuf;
use std::sync::OnceLock;
use tauri::{AppHandle, Manager, Runtime};

const DATABASE_VERSION: &str = "0.3";

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

const SETTINGS_FILE_NAME: &str = "settings.json";
#[cfg(desktop)]
const LOCAL_MAILBOX_DB_FILE_NAME: &str = "local-mailbox.redb";
#[cfg(desktop)]
const DASHCHAT_DATA_FOLDER: &str = "studio.darksoil.dashchat";

/// Manages paths within the versioned Dash Chat data directory.
pub struct FileSystem {
    app_root_dir: PathBuf,
    app_data_dir: PathBuf,
}

impl FileSystem {
    /// Create from a Tauri `AppHandle`, resolving the base data directory
    /// from `DATA_DIR` env var or the OS local data dir.
    pub fn new<R: Runtime>(handle: &AppHandle<R>) -> anyhow::Result<Self> {
        let app_root_dir = if let Ok(data_dir) = std::env::var("DATA_DIR") {
            PathBuf::from(data_dir)
        } else {
            // Linux: local_data_dir() = ".local/share"
            // Mobile: local_data_dir() = private app data
            let local_data_dir = handle.path().local_data_dir()?;

            // In desktop, store all app data in ".local/share/studio.darksoil.dashchat"
            // In mobile, no need to add the app folder because local_data_dir is not shared with any other app
            #[cfg(desktop)]
            let local_data_dir = local_data_dir.join(DASHCHAT_DATA_FOLDER);

            local_data_dir
        };
        Self::from_app_root_dir(app_root_dir)
    }

    /// Create from a raw data directory path (e.g. from a push notification context).
    pub fn from_app_root_dir(app_root_dir: PathBuf) -> anyhow::Result<Self> {
        // We don't support backwards compatibility yet
        // Store all files in a separate folder for each version of the database
        // to force a reset from scratch in the state of app on app updates
        let app_data_dir = app_root_dir.join(DATABASE_VERSION);
        if !app_data_dir.exists() {
            std::fs::create_dir_all(&app_data_dir)?;
        }
        Ok(FileSystem {
            app_root_dir,
            app_data_dir,
        })
    }

    pub fn app_root_dir(&self) -> &PathBuf {
        &self.app_root_dir
    }

    // The folder where all the data files for the app should be stored
    pub fn app_data_dir(&self) -> &PathBuf {
        &self.app_data_dir
    }

    /// The folder where logs should be stored
    /// Sibling of `app_data_dir` (NOT versioned)
    pub fn logs_dir(&self) -> PathBuf {
        self.app_root_dir.join("logs")
    }

    pub fn settings_path(&self) -> PathBuf {
        self.app_data_dir.join(SETTINGS_FILE_NAME)
    }

    #[cfg(desktop)]
    pub fn local_mailbox_db_path(&self) -> PathBuf {
        self.app_data_dir.join(LOCAL_MAILBOX_DB_FILE_NAME)
    }
}
