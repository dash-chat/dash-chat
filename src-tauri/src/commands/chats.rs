use dashchat_node::{
    AgentId, ChatId, ChatReaction, DeviceId, GroupInfo, OutgoingMedia, RemoveGroupMemberError,
};
use p2panda_auth::{Access, AccessLevel};
use p2panda_core::Hash;
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::app_node::AppNode;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GroupMember {
    pub agent_id: AgentId,
    pub device_ids: Vec<DeviceId>,
    pub is_admin: bool,
}

#[tauri::command]
pub async fn create_group(
    initial_members: Vec<AgentId>,
    app_node: State<'_, AppNode>,
) -> Result<ChatId, String> {
    let node = app_node.get().await?;
    let mut members = std::collections::BTreeMap::new();
    for agent_id in initial_members {
        let device_id = node
            .local_store
            .lookup_contact_by_agent_id(agent_id)
            .await
            .map_err(|e| format!("Failed to look up contact: {e:?}"))?
            .ok_or_else(|| format!("No device found for agent {:?}", agent_id))?;
        members.insert(*device_id, Access::write());
    }
    node.create_group(members)
        .await
        .map_err(|e| format!("Failed to create group: {e:?}"))
}

#[tauri::command]
pub async fn set_group_info(
    chat_id: ChatId,
    info: GroupInfo,
    app_node: State<'_, AppNode>,
) -> Result<(), String> {
    let node = app_node.get().await?;
    node.set_group_info(chat_id, info)
        .await
        .map_err(|e| format!("Failed to set group info: {e:?}"))?;
    Ok(())
}

#[tauri::command]
pub async fn add_group_member(
    chat_id: ChatId,
    agent_id: AgentId,
    app_node: State<'_, AppNode>,
) -> Result<(), String> {
    let node = app_node.get().await?;
    let device_id = node
        .local_store
        .lookup_contact_by_agent_id(agent_id)
        .await
        .map_err(|e| format!("Failed to look up contact: {e:?}"))?
        .ok_or_else(|| format!("No device found for agent {:?}", agent_id))?;
    node.add_group_member(chat_id, *device_id, Access::write())
        .await
        .map_err(|e| format!("Failed to add group member: {e:?}"))
}

#[tauri::command]
pub async fn send_message(
    chat_id: ChatId,
    message: String,
    media: Option<OutgoingMedia>,
    reply: Option<Hash>,
    app_node: State<'_, AppNode>,
) -> Result<Hash, String> {
    let node = app_node.get().await?;
    let header = node
        .send_message(chat_id, message, media, reply)
        .await
        .map_err(|err| format!("{err:?}"))?;
    Ok(header.hash())
}

#[tauri::command]
pub async fn edit_message(
    chat_id: ChatId,
    edit_hash: Hash,
    message: String,
    app_node: State<'_, AppNode>,
) -> Result<Hash, String> {
    let node = app_node.get().await?;
    let header = node
        .edit_message(chat_id, edit_hash, message)
        .await
        .map_err(|err| format!("{err:?}"))?;
    Ok(header.hash())
}

#[tauri::command]
pub async fn delete_message(
    chat_id: ChatId,
    target_hash: Hash,
    app_node: State<'_, AppNode>,
) -> Result<Hash, String> {
    let node = app_node.get().await?;
    let header = node
        .delete_message(chat_id, target_hash)
        .await
        .map_err(|err| format!("{err:?}"))?;
    Ok(header.hash())
}

#[tauri::command]
pub async fn delete_message_for_me(
    chat_id: ChatId,
    target_hash: Hash,
    app_node: State<'_, AppNode>,
) -> Result<Hash, String> {
    let node = app_node.get().await?;
    let header = node
        .delete_message_for_me(chat_id, target_hash)
        .await
        .map_err(|err| format!("{err:?}"))?;
    Ok(header.hash())
}

#[tauri::command]
pub async fn send_reaction(
    chat_id: ChatId,
    content: ChatReaction,
    app_node: State<'_, AppNode>,
) -> Result<(), String> {
    let node = app_node.get().await?;
    node.add_reaction(chat_id, content)
        .await
        .map_err(|err| format!("{err:?}"))?;
    Ok(())
}

#[tauri::command]
pub async fn mark_messages_read(
    chat_id: ChatId,
    message_hashes: Vec<Hash>,
    app_node: State<'_, AppNode>,
) -> Result<(), String> {
    let node = app_node.get().await?;
    node.mark_messages_read(chat_id, message_hashes)
        .await
        .map_err(|e| format!("Failed to mark messages as read: {e:?}"))
}

#[tauri::command]
pub async fn get_group_chats(app_node: State<'_, AppNode>) -> Result<Vec<ChatId>, String> {
    let node = app_node.get().await?;
    node.get_groups()
        .await
        .map_err(|e| format!("Failed to get groups: {e:?}"))
}

#[tauri::command]
pub async fn get_group_members(
    chat_id: ChatId,
    app_node: State<'_, AppNode>,
) -> Result<Vec<GroupMember>, String> {
    let node = app_node.get().await?;
    let members = node
        .get_group_members(chat_id)
        .await
        .map_err(|e| format!("Failed to get group members: {e:?}"))?;
    let my_device_id = node.device_id();
    let my_agent_id = node.agent_id();
    let mut grouped: std::collections::BTreeMap<AgentId, GroupMember> =
        std::collections::BTreeMap::new();
    for (device_id, access) in members {
        let agent_id = if device_id == my_device_id {
            my_agent_id
        } else {
            node.local_store
                .lookup_contact_by_device_id(device_id)
                .await
                .map_err(|e| format!("Failed to lookup contact: {e:?}"))?
                .unwrap_or_else(|| {
                    AgentId::from_bytes(device_id.as_bytes())
                        .expect("DeviceId is a valid 32-byte key")
                })
        };
        let is_admin = access.level >= AccessLevel::Manage;
        let entry = grouped.entry(agent_id).or_insert_with(|| GroupMember {
            agent_id,
            device_ids: Vec::new(),
            is_admin: false,
        });
        entry.device_ids.push(device_id);
        entry.is_admin |= is_admin;
    }
    Ok(grouped.into_values().collect())
}

#[tauri::command]
pub async fn remove_group_member(
    chat_id: ChatId,
    agent_id: AgentId,
    app_node: State<'_, AppNode>,
) -> Result<(), String> {
    let node = app_node.get().await?;
    let device_id = node
        .local_store
        .lookup_contact_by_agent_id(agent_id)
        .await
        .map_err(|e| format!("{e:?}"))?
        .ok_or_else(|| format!("No device found for agent {:?}", agent_id))?;
    node.remove_group_member(chat_id, *device_id)
        .await
        .map_err(|e| format!("{e:?}"))
}

#[tauri::command]
pub async fn leave_group(
    chat_id: ChatId,
    app_node: State<'_, AppNode>,
) -> Result<(), RemoveGroupMemberError> {
    let node = app_node
        .get()
        .await
        .map_err(|e| RemoveGroupMemberError::from(anyhow::anyhow!(e)))?;
    let my_device_id = node.device_id();
    node.remove_group_member(chat_id, *my_device_id)
        .await
        .map_err(RemoveGroupMemberError::from)
}
