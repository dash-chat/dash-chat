use dashchat_node::{Error, Node, Profile};
use tauri::State;

#[tauri::command]
pub async fn set_profile(profile: Profile, node: State<'_, Node>) -> Result<(), Error> {
    node.set_profile(profile).await
}
