use dashchat_node::Profile;
use tauri::State;

use crate::error::Error;
use crate::node::AppNode;

#[tauri::command]
pub async fn set_profile(profile: Profile, app_node: State<'_, AppNode>) -> Result<(), Error> {
    let node = app_node.get().await?;
    node.set_profile(profile).await?;
    Ok(())
}
