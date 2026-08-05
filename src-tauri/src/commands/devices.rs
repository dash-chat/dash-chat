use dashchat_node::{topic::kind, Topic};
use tauri::State;

use crate::node::AppNode;

#[tauri::command]
pub async fn my_device_group_topic(
    app_node: State<'_, AppNode>,
) -> Result<Topic<kind::DeviceGroup>, String> {
    let node = app_node.get().await?;
    Ok(node.device_group_topic())
}
