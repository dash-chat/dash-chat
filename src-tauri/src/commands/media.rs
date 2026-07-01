use std::time::Duration;
use tauri::{AppHandle, Manager, State};

use crate::app_node::AppNode;

/// Load a blob from the node's local store and write it to a file in the app
/// cache directory, returning the absolute path.
#[tauri::command]
pub async fn save_blob_to_cache(
    hash: String,
    name: String,
    app: AppHandle,
    app_node: State<'_, AppNode>,
) -> Result<String, String> {
    let node = app_node.get().await?;
    let dir = app
        .path()
        .app_cache_dir()
        .map_err(|e| format!("Failed to resolve app cache dir: {e:?}"))?
        .join(sanitized_file_name(&hash));
    let path = dir.join(sanitized_file_name(&name));

    if !path.exists() {
        let bytes = node
            .load_blob(&hash, Some(Duration::from_secs(30)))
            .await
            .map_err(|e| format!("Failed to load blob {hash}: {e:?}"))?;

        tokio::fs::create_dir_all(&dir)
            .await
            .map_err(|e| format!("Failed to create cache dir: {e:?}"))?;
        tokio::fs::write(&path, &bytes)
            .await
            .map_err(|e| format!("Failed to write cache file: {e:?}"))?;
    }

    path.to_str()
        .map(str::to_string)
        .ok_or_else(|| "Cache file path is not valid UTF-8".to_string())
}

/// Drop any directory components from a peer-supplied name so the written file
/// can never escape the cache directory.
fn sanitized_file_name(name: &str) -> String {
    name.rsplit(['/', '\\'])
        .next()
        .filter(|s| !s.is_empty() && *s != "." && *s != "..")
        .unwrap_or("attachment")
        .to_string()
}
