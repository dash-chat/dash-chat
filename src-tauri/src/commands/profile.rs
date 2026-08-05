use dashchat_node::Profile;
use tauri::State;

use crate::node::AppNode;
use crate::error::Error;

#[tauri::command]
pub async fn set_profile(profile: Profile, app_node: State<'_, AppNode>) -> Result<(), Error> {
    let node = app_node.get().await?;
    node.set_profile(profile).await?;
    Ok(())
}
