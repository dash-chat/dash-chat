#[cfg(feature = "e2e-tests")]
use crate::node::AppNodeManager;
#[cfg(feature = "e2e-tests")]
use tauri::State;

/// Pause or resume the background blob fetch loop, so a spec can observe an
/// attachment in its downloading state. Only registered under `e2e-tests`.
#[tauri::command]
#[cfg(feature = "e2e-tests")]
pub async fn set_blob_fetch_paused(
    paused: bool,
    app_node_manager: State<'_, AppNodeManager>,
) -> Result<(), String> {
    let node = app_node_manager.get().await?;
    node.set_blob_fetch_paused(paused).await;
    Ok(())
}
