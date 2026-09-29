use tauri::State;

use crate::node::AppNodeManager;

/// Pause or resume blob fetching (background loop and on-demand), so a spec
/// can observe an attachment in its downloading state.
#[tauri::command]
pub async fn set_blob_fetch_paused(
    paused: bool,
    app_node_manager: State<'_, AppNodeManager>,
) -> Result<(), String> {
    let node = app_node_manager.get().await?;
    node.set_blob_fetch_paused(paused);
    Ok(())
}
