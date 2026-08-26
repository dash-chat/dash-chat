use dashchat_node::{topic::kind::Chat, FakeAgentId, Topic};
use tauri::State;

use crate::node::AppNodeManager;

#[tauri::command]
pub async fn direct_chat_id(
    peer: FakeAgentId,
    app_node_manager: State<'_, AppNodeManager>,
) -> Result<Topic<Chat>, String> {
    let node = app_node_manager.get().await?;
    Ok(Topic::direct_chat([node.fake_agent_id(), peer]))
}
