use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, Runtime};

const SETTINGS_FILE_NAME: &str = "settings.json";

#[derive(Debug, Default, Serialize, Deserialize)]
struct Settings {
    local_mailbox_enabled: bool,
}

fn settings_path<R: Runtime>(handle: &AppHandle<R>) -> anyhow::Result<PathBuf> {
    Ok(handle.path().local_data_dir()?.join(SETTINGS_FILE_NAME))
}

pub fn load_mailbox_enabled<R: Runtime>(handle: &AppHandle<R>) -> bool {
    let path = match settings_path(handle) {
        Ok(path) => path,
        Err(err) => {
            log::error!("Failed to resolve settings path: {err:?}");
            return false;
        }
    };

    let contents = match fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return false,
        Err(err) => {
            log::error!("Failed to read settings file at {path:?}: {err:?}");
            return false;
        }
    };

    match serde_json::from_str::<Settings>(&contents) {
        Ok(settings) => settings.local_mailbox_enabled,
        Err(err) => {
            log::error!("Failed to parse settings file at {path:?}: {err:?}");
            false
        }
    }
}

pub fn save_mailbox_enabled<R: Runtime>(handle: &AppHandle<R>, enabled: bool) {
    let path = match settings_path(handle) {
        Ok(path) => path,
        Err(err) => {
            log::error!("Failed to resolve settings path: {err:?}");
            return;
        }
    };

    if let Some(parent) = path.parent() {
        if let Err(err) = fs::create_dir_all(parent) {
            log::error!("Failed to create settings directory {parent:?}: {err:?}");
            return;
        }
    }

    let settings = Settings {
        local_mailbox_enabled: enabled,
    };

    let contents = match serde_json::to_string_pretty(&settings) {
        Ok(contents) => contents,
        Err(err) => {
            log::error!("Failed to serialize settings: {err:?}");
            return;
        }
    };

    if let Err(err) = fs::write(&path, contents) {
        log::error!("Failed to write settings file at {path:?}: {err:?}");
    }
}
