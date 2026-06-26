use dashchat_node::Node;
use std::time::Duration;
use tauri::{AppHandle, Manager, State};

/// Load a blob from the node's local store and write it to a file in the app
/// cache directory, returning the absolute path.
///
/// Used to hand a downloaded attachment to the system file viewer without
/// round-tripping its bytes back through the JS↔Rust IPC bridge: a plugin-fs
/// `writeFile` of a multi-MB attachment runs at ~1 MB/s on Android, so doing
/// the load-and-write entirely in Rust keeps the bytes off the bridge.
#[tauri::command]
pub async fn save_blob_to_cache(
    hash: String,
    name: String,
    app: AppHandle,
    node: State<'_, Node>,
) -> Result<String, String> {
    let bytes = node
        .load_blob(&hash, Some(Duration::from_secs(30)))
        .await
        .map_err(|e| format!("Failed to load blob {hash}: {e:?}"))?;

    let dir = app
        .path()
        .app_cache_dir()
        .map_err(|e| format!("Failed to resolve app cache dir: {e:?}"))?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("Failed to create cache dir: {e:?}"))?;

    let path = dir.join(sanitized_file_name(&name));
    std::fs::write(&path, &bytes).map_err(|e| format!("Failed to write cache file: {e:?}"))?;

    path.to_str()
        .map(str::to_string)
        .ok_or_else(|| "Cache file path is not valid UTF-8".to_string())
}

/// Drop any directory components from a peer-supplied name so the written file
/// can never escape the cache directory.
fn sanitized_file_name(name: &str) -> String {
    name.rsplit(['/', '\\'])
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or("attachment")
        .to_string()
}
