use dashchat_node::{topic::kind::Chat, AgentId, Topic};
use tauri::State;

use crate::app_node::AppNode;

#[tauri::command]
pub async fn direct_chat_id(
    peer: AgentId,
    app_node: State<'_, AppNode>,
) -> Result<Topic<Chat>, String> {
    let node = app_node.get().await?;
    Ok(Topic::direct_chat([node.agent_id(), peer]))
}
