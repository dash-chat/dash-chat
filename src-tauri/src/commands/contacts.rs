use dashchat_node::{topic::kind::Inbox, AddContactQrCode, AgentId, DeviceId, Topic};
use std::{collections::BTreeSet, str::FromStr};
use tauri::State;

use crate::error::Error;
use crate::node::AppNode;

#[tauri::command]
pub async fn create_contact_code(app_node: State<'_, AppNode>) -> Result<String, Error> {
    let node = app_node.get().await?;
    Ok(node.create_add_contact_qr_code().await?.to_string())
}

#[tauri::command]
pub async fn my_agent_id(app_node: State<'_, AppNode>) -> Result<AgentId, String> {
    let node = app_node.get().await?;
    Ok(node.agent_id())
}

#[tauri::command]
pub async fn my_device_id(app_node: State<'_, AppNode>) -> Result<DeviceId, String> {
    let node = app_node.get().await?;
    Ok(node.device_id())
}

#[tauri::command]
pub async fn agent_for_device(
    device_pubkey: DeviceId,
    app_node: State<'_, AppNode>,
) -> Result<Option<AgentId>, Error> {
    let node = app_node.get().await?;
    Ok(node
        .lookup_contact(device_pubkey)
        .await
        .map_err(|e| dashchat_node::Error::AuthorOperation(e.to_string()))?)
}

#[tauri::command]
pub async fn add_contact(
    contact_code: String,
    app_node: State<'_, AppNode>,
) -> Result<DeviceId, Error> {
    let qr = AddContactQrCode::from_str(&contact_code)
        .map_err(|e| dashchat_node::AddContactError::InvalidContactCode(e.to_string()))?;
    let device_pubkey = qr.device_pubkey;
    let node = app_node.get().await?;
    node.add_contact(qr).await?;
    Ok(device_pubkey)
}

#[tauri::command]
pub async fn accept_contact(agent_id: AgentId, app_node: State<'_, AppNode>) -> Result<(), Error> {
    let node = app_node.get().await?;
    node.accept_contact(agent_id).await?;
    Ok(())
}

#[tauri::command]
pub async fn active_inbox_topics(
    app_node: State<'_, AppNode>,
) -> Result<BTreeSet<Topic<Inbox>>, Error> {
    let node = app_node.get().await?;
    let topics = node.get_active_inbox_topics().await?;
    let topics_ids = topics.clone().into_iter().map(|t| t.topic).collect();

    Ok(topics_ids)
}

#[tauri::command]
pub async fn block_contact(agent_id: AgentId, app_node: State<'_, AppNode>) -> Result<(), Error> {
    let node = app_node.get().await?;
    Ok(node.block_contact(agent_id).await?)
}

#[tauri::command]
pub async fn report_contact(agent_id: AgentId, app_node: State<'_, AppNode>) -> Result<(), Error> {
    let node = app_node.get().await?;
    node.report_contact(agent_id)
        .await
        .map_err(|e| dashchat_node::Error::AuthorOperation(e.to_string()))?;
    Ok(())
}

#[tauri::command]
pub async fn unblock_contact(agent_id: AgentId, app_node: State<'_, AppNode>) -> Result<(), Error> {
    let node = app_node.get().await?;
    Ok(node.unblock_contact(agent_id).await?)
}

// #[tauri::command]
// pub async fn remove_contact(
//     contact_id: VerifyingKey,
//     node: State<'_, Node>,
// ) -> Result<VerifyingKey, String> {
//     node.remove_contact(contact_id.into())
//         .await
//         .map_err(|e| format!("Failed to remove contact: {e:?}"))
// }

// #[tauri::command]
// pub async fn get_contacts(node: State<'_, Node>) -> Result<Vec<VerifyingKey>, String> {
//     let pks = node
//         .get_contacts()
//         .await
//         .map_err(|e| format!("Failed to get my contacts: {e:?}"))?;

//     let pks = pks.into_iter().map(|pk| pk.into()).collect();

//     Ok(pks)
// }
