use std::fs;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Runtime};

use crate::filesystem::FileSystem;

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Settings {
    pub local_mailbox_enabled: bool,
    pub qr_color: Option<String>,
    pub color_scheme: Option<String>,
}

pub(crate) fn load_settings<R: Runtime>(handle: &AppHandle<R>) -> Settings {
    let path = match FileSystem::new(handle).settings_path() {
        Ok(path) => path,
        Err(err) => {
            log::error!("Failed to resolve settings path: {err:?}");
            return Settings::default();
        }
    };

    let contents = match fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Settings::default(),
        Err(err) => {
            log::error!("Failed to read settings file at {path:?}: {err:?}");
            return Settings::default();
        }
    };

    match serde_json::from_str::<Settings>(&contents) {
        Ok(settings) => settings,
        Err(err) => {
            log::error!("Failed to parse settings file at {path:?}: {err:?}");
            Settings::default()
        }
    }
}

pub(crate) fn save_settings<R: Runtime>(handle: &AppHandle<R>, settings: &Settings) {
    let path = match FileSystem::new(handle).settings_path() {
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

    let contents = match serde_json::to_string_pretty(settings) {
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

pub fn load_mailbox_enabled<R: Runtime>(handle: &AppHandle<R>) -> bool {
    load_settings(handle).local_mailbox_enabled
}

pub fn save_mailbox_enabled<R: Runtime>(handle: &AppHandle<R>, enabled: bool) {
    let mut settings = load_settings(handle);
    settings.local_mailbox_enabled = enabled;
    save_settings(handle, &settings);
}
