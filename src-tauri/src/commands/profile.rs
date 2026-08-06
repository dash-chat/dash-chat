use dashchat_node::Profile;
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
