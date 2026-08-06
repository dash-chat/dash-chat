use dashchat_node::{topic::kind, Topic};
use tauri::State;

use crate::node::AppNodeManager;

#[tauri::command]
pub async fn my_device_group_topic(
    app_node_manager: State<'_, AppNodeManager>,
) -> Result<Topic<kind::DeviceGroup>, String> {
    let node = app_node_manager.get().await?;
    Ok(node.device_group_topic())
}
