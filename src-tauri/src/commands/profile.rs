use dashchat_node::Profile;
use tauri::State;

use crate::app_node::AppNode;
use crate::error::Error;

#[tauri::command]
pub async fn set_profile(profile: Profile, app_node: State<'_, AppNode>) -> Result<(), Error> {
    let node = app_node.get().await.map_err(Error::NodeNotReady)?;
    node.set_profile(profile).await?;
    Ok(())
}
