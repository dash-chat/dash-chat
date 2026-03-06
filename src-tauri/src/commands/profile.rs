use dashchat_node::{Error, Node, Profile};
use tauri::{AppHandle, Manager, State};

#[tauri::command]
pub async fn set_profile(profile: Profile, node: State<'_, Node>) -> Result<(), Error> {
    node.set_profile(profile).await
}

#[tauri::command]
pub async fn delete_profile(app: AppHandle, node: State<'_, Node>) -> Result<(), Error> {
    node.delete_profile();
    tauri::process::restart(&app.env());
}