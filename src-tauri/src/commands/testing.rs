#[cfg(feature = "e2e-tests")]
use crate::node::AppNode;
#[cfg(feature = "e2e-tests")]
use tauri::State;

/// Close this node's iroh endpoint so it can no longer sync with peers over
/// p2p. Local reads/writes keep working. Only registered under the
/// `e2e-tests` feature; used by specs that must observe pre-sync UI state
/// without racing a direct p2p connection between two agents on one machine.
#[tauri::command]
#[cfg(feature = "e2e-tests")]
pub async fn close_iroh_endpoint(app_node: State<'_, AppNode>) -> Result<(), String> {
    let node = app_node.get().await?;
    node.close_iroh_endpoint().await.map_err(|e| e.to_string())
}
