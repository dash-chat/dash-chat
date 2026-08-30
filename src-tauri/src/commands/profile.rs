use dashchat_node::{AgentId, Profile};
use tauri::State;

use crate::error::Error;
use crate::node::AppNodeManager;

#[tauri::command]
pub async fn set_profile(
    profile: Profile,
    app_node_manager: State<'_, AppNodeManager>,
) -> Result<(), Error> {
    let node = app_node_manager.get().await?;
    node.set_profile(profile).await?;
    Ok(())
}

#[tauri::command]
pub async fn get_profile(
    agent_id: AgentId,
    app_node_manager: State<'_, AppNodeManager>,
) -> Result<Option<Profile>, Error> {
    let node = app_node_manager.get().await?;
    Ok(node
        .get_profile(agent_id)
        .await
        .map_err(|e| dashchat_node::Error::AuthorOperation(e.to_string()))?)
}
