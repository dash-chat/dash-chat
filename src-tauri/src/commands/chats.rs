use dashchat_node::{AgentId, ChatId, DeviceId, Node};
use p2panda_auth::{Access, AccessLevel};
use p2panda_core::{Hash, PublicKey};
use tauri::State;

#[tauri::command]
pub async fn create_group(
    initial_members: Vec<PublicKey>,
    node: State<'_, Node>,
) -> Result<ChatId, String> {
    let members = initial_members
        .into_iter()
        .map(|pk| (pk, Access::write()))
        .collect();
    node.create_group(members)
        .await
        .map_err(|e| format!("Failed to create group: {e:?}"))
}

#[tauri::command]
pub async fn mark_messages_read(
    chat_id: ChatId,
    message_hashes: Vec<Hash>,
    node: State<'_, Node>,
) -> Result<(), String> {
    node.mark_messages_read(chat_id, message_hashes)
        .await
        .map_err(|e| format!("Failed to mark messages as read: {e:?}"))
}

// #[command]
// pub async fn create_group_chat(group_chat_id: GroupChatId, node: State<'_, Node>) -> Result<(), String> {
//     node.create_group_chat_space(group_chat_id)
//         .await
//         .map_err(|e| format!("Failed to create group: {e:?}"))
// }

#[tauri::command]
pub async fn get_group_chats(node: State<'_, Node>) -> Result<Vec<ChatId>, String> {
    node.get_groups()
        .await
        .map_err(|e| format!("Failed to get groups: {e:?}"))
}

#[tauri::command]
pub async fn get_group_members(
    chat_id: ChatId,
    node: State<'_, Node>,
) -> Result<Vec<(AgentId, bool)>, String> {
    let members = node
        .get_group_members(chat_id)
        .await
        .map_err(|e| format!("Failed to get group members: {e:?}"))?;
    let my_device_id = node.device_id();
    let my_agent_id = node.agent_id();
    let mut result = Vec::with_capacity(members.len());
    for (device_id, access) in members {
        let agent_id = if device_id == my_device_id {
            my_agent_id
        } else {
            node.local_store
                .lookup_contact_by_device_id(device_id)
                .await
                .map_err(|e| format!("Failed to lookup contact: {e:?}"))?
                .unwrap_or_else(|| AgentId::from_bytes(device_id.as_bytes()).expect("DeviceId is a valid 32-byte key"))
        };
        result.push((agent_id, access.level >= AccessLevel::Manage));
    }
    Ok(result)
}
