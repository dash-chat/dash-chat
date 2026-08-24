use dashchat_node::{topic::kind::Chat, AgentId, Topic};
use tauri::State;

use crate::node::AppNodeManager;

#[tauri::command]
pub async fn direct_chat_id(
    peer: AgentId,
    app_node_manager: State<'_, AppNodeManager>,
) -> Result<Topic<Chat>, String> {
    let node = app_node_manager.get().await?;
    Ok(Topic::direct_chat([node.agent_id(), peer]))
}
