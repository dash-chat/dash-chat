#[cfg(feature = "e2e-tests")]
use crate::node::AppNodeManager;
#[cfg(feature = "e2e-tests")]
use tauri::State;

/// Close this node's iroh endpoint so it can no longer sync with peers over
/// p2p. Local reads/writes keep working. Only registered under the
/// `e2e-tests` feature; used by specs that must observe pre-sync UI state
/// without racing a direct p2p connection between two agents on one machine.
#[tauri::command]
#[cfg(feature = "e2e-tests")]
pub async fn close_iroh_endpoint(
    app_node_manager: State<'_, AppNodeManager>,
) -> Result<(), String> {
    let node = app_node_manager.get().await?;
    node.close_iroh_endpoint().await.map_err(|e| e.to_string())
}

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
