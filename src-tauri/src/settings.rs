use std::fs;

use crate::filesystem::FileSystem;
use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Runtime};

#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Settings {
    pub local_mailbox_enabled: bool,
    #[serde(default = "default_notifications_enabled")]
    pub notifications_enabled: bool,
    pub qr_color: Option<String>,
    pub background_mode_enabled: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            local_mailbox_enabled: false,
            notifications_enabled: default_notifications_enabled(),
            qr_color: None,
            background_mode_enabled: false,
        }
    }
}

// The app-level toggle only exists on desktop; on mobile the OS-level
// permission is the single source of truth, so default the flag to ON there
// too and skip the app-level check when deciding to notify.
const fn default_notifications_enabled() -> bool {
    true
}

pub(crate) fn load_settings<R: Runtime>(handle: &AppHandle<R>) -> Settings {
    let path = match FileSystem::new(handle) {
        Ok(fs) => fs.settings_path(),
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

pub(crate) fn set_setting<R: Runtime>(
    handle: &AppHandle<R>,
    key: String,
    value: serde_json::Value,
) -> anyhow::Result<()> {
    let mut current = serde_json::to_value(load_settings(handle)).unwrap_or_default();

    let known_keys = current
        .as_object()
        .map(|obj| obj.keys().cloned().collect::<Vec<_>>())
        .unwrap_or_default();

    if !known_keys.contains(&key) {
        return Err(anyhow!("Unknown setting: {key}"));
    }

    if let Some(obj) = current.as_object_mut() {
        obj.insert(key.clone(), value.clone());
    }

    let settings = serde_json::from_value::<Settings>(current)
        .map_err(|err| anyhow!("Invalid setting {key}: {err}"))?;

    save_settings(&handle, &settings);
    handle.emit(format!("settings://updated-{key}").as_str(), value)?;

    Ok(())
}

pub(crate) fn save_settings<R: Runtime>(handle: &AppHandle<R>, settings: &Settings) {
    let path = match FileSystem::new(handle) {
        Ok(fs) => fs.settings_path(),
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

    if let Err(err) = fs::write(&path, &contents) {
        log::error!("Failed to write settings file at {path:?}: {err:?}");
    }

    // Notify the frontend so reactive stores pick up the change.
    if let Ok(updated) = serde_json::to_value(settings) {
        let _ = handle.emit("settings://updated", updated);
    }
}

#[cfg(desktop)]
pub fn load_mailbox_enabled<R: Runtime>(handle: &AppHandle<R>) -> bool {
    load_settings(handle).local_mailbox_enabled
}

#[cfg(desktop)]
pub fn save_mailbox_enabled<R: Runtime>(handle: &AppHandle<R>, enabled: bool) {
    let mut settings = load_settings(handle);
    settings.local_mailbox_enabled = enabled;
    save_settings(handle, &settings);
}
