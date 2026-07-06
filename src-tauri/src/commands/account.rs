#[cfg(mobile)]
use dashchat_node::Node;
use tauri::Manager;
use tauri::{AppHandle, State};

use crate::app_node::AppNode;

#[tauri::command]
pub async fn delete_account(app: AppHandle, app_node: State<'_, AppNode>) -> Result<(), String> {
    log::info!("Deleting account...");

    let node = app_node.get().await?;

    #[cfg(mobile)]
    unregister_fcm_token(&app, &node).await;

    node.shutdown().await.map_err(|e| {
        log::error!("Failed to shutdown node while trying to delete account: {e:?}");
        String::from("Failed to delete account.")
    })?;

    // Past this point the node is shut down and the app is unrecoverable: DB pools
    // are closed and any further command would fail. We must always exit/restart,
    // even if data-dir deletion partially fails. Any leftover files are surfaced
    // via logs only.

    #[cfg(desktop)]
    if let Err(e) = crate::mailbox::server::stop_local_mailbox(&app).await {
        log::error!("Failed to stop local mailbox while trying to delete account: {e:?}");
    }

    let data_path = node.data_path();
    if let Err(e) = std::fs::remove_dir_all(&data_path) {
        log::error!("Failed to delete account data dir, retrying once: {e}");
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        if let Err(e) = std::fs::remove_dir_all(&data_path) {
            log::error!("Failed to delete account data dir after retry: {e}");
        } else {
            log::info!("Account data dir deleted on retry");
        }
    } else {
        log::info!("Account deleted successfully");
    }

    #[cfg(mobile)]
    {
        app.exit(0);
        Ok(())
    }

    #[cfg(desktop)]
    {
        tauri::process::restart(&app.env());
    }
}

#[cfg(mobile)]
async fn unregister_fcm_token(app: &AppHandle, node: &Node) {
    use push_notifications_client::client::PushNotificationsClient;
    use push_notifications_client::types::VerifyingKey;

    let client = app.state::<PushNotificationsClient>();
    let verifying_key = VerifyingKey::from(node.device_id().to_string());
    match client.unregister_fcm_token(verifying_key).await {
        Ok(()) => log::info!("Unregistered FCM token from push notifications server."),
        Err(e) => log::error!("Failed to unregister FCM token: {e:?}"),
    }
}
