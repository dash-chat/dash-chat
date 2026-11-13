use dashchat_node::{topic::kind::Chat, DeviceId, FakeAgentId, Topic};
use tauri::State;

use crate::node::AppNodeManager;

#[tauri::command]
pub async fn direct_chat_id(
    peer: FakeAgentId,
    app_node_manager: State<'_, AppNodeManager>,
) -> Result<Topic<Chat>, String> {
    let node = app_node_manager.get().await?;
    node.direct_chat_id(DeviceId::from(peer))
        .await
        .map_err(|err| err.to_string())?
        .ok_or_else(|| "no contact request was exchanged with this device".to_string())
}
