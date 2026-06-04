use dashchat_node::{topic::kind::Chat, AgentId, Node, Topic};
use tauri::State;

#[tauri::command]
pub fn direct_chat_id(peer: AgentId, node: State<'_, Node>) -> Topic<Chat> {
    Topic::direct_chat([node.agent_id(), peer])
}
