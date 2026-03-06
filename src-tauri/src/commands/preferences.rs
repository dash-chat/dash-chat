use std::collections::HashMap;
use std::fs;

use dashchat_node::Node;
use serde_json::Value;
use tauri::State;

/// Preferences are stored as freeform JSON, we do simple validation in the frontend store

#[tauri::command]
pub async fn get_preferences(
    node: State<'_, Node>,
) -> Result<serde_json::Map<String, Value>, String> {
    let preferences_file = fs::read_to_string(node.filesystem.preferences_path());
    if let Ok(contents) = preferences_file {
        if let Ok(val) = serde_json::from_str::<serde_json::Map<String, Value>>(&contents) {
            return Ok(val);
        }
    }
    Ok(serde_json::Map::new())
}

#[tauri::command]
pub fn set_preferences(
    preferences: HashMap<String, Value>,
    node: State<'_, Node>,
) -> Result<(), String> {
    let s = serde_json::to_string(&preferences)
        .map_err(|_| "Couldn't serialize preferences as JSON")?;
    let _ = fs::write(node.filesystem.preferences_path(), s);
    Ok(())
}
