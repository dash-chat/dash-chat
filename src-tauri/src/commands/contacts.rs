use dashchat_node::{
    topic::kind::Inbox, AddContactError, AgentId, ContactCode, ContactCodeError, Error, Node, Topic,
};
use std::collections::BTreeSet;
use tauri::State;

#[tauri::command]
pub async fn get_or_create_contact_code(
    node: State<'_, Node>,
) -> Result<ContactCode, ContactCodeError> {
    node.get_or_create_contact_code().await
}

#[tauri::command]
pub async fn reset_contact_code(node: State<'_, Node>) -> Result<ContactCode, ContactCodeError> {
    node.reset_contact_code().await
}

#[tauri::command]
pub fn my_agent_id(node: State<'_, Node>) -> AgentId {
    node.agent_id()
}

#[tauri::command]
pub async fn add_contact(
    contact_code: ContactCode,
    node: State<'_, Node>,
) -> Result<(), AddContactError> {
    node.add_contact(contact_code).await?;
    Ok(())
}

#[tauri::command]
pub fn active_inbox_topics(node: State<'_, Node>) -> Result<BTreeSet<Topic<Inbox>>, Error> {
    let topics = node.get_active_inbox_topics()?;
    let topics_ids = topics.clone().into_iter().map(|t| t.topic).collect();

    Ok(topics_ids)
}

#[tauri::command]
pub async fn reject_contact_request(agent_id: AgentId, node: State<'_, Node>) -> Result<(), Error> {
    node.reject_contact_request(agent_id).await
}

// #[tauri::command]
// pub async fn remove_contact(
//     contact_id: PublicKey,
//     node: State<'_, Node>,
// ) -> Result<PublicKey, String> {
//     node.remove_contact(contact_id.into())
//         .await
//         .map_err(|e| format!("Failed to remove contact: {e:?}"))
// }

// #[tauri::command]
// pub async fn get_contacts(node: State<'_, Node>) -> Result<Vec<PublicKey>, String> {
//     let pks = node
//         .get_contacts()
//         .await
//         .map_err(|e| format!("Failed to get my contacts: {e:?}"))?;

//     let pks = pks.into_iter().map(|pk| pk.into()).collect();

//     Ok(pks)
// }
