use dashchat_node::Node;
#[cfg(not(mobile))]
use tauri::Manager;
use tauri::{AppHandle, State};

#[tauri::command]
pub async fn delete_account(app: AppHandle, node: State<'_, Node>) -> Result<(), String> {
    log::info!("Deleting account...");

    node.shutdown().await.map_err(|e| {
        log::error!("Failed to shutdown node while trying to delete account: {e:?}");
        String::from("Failed to delete account.")
    })?;

    std::fs::remove_dir_all(node.data_path()).map_err(|e| {
        log::error!("Failed to delete account: {e}");
        String::from("Failed to delete account.")
    })?;
    log::info!("Account deleted successfully");

    #[cfg(mobile)]
    {
        app.exit(0);
        Ok(())
    }

    #[cfg(not(mobile))]
    {
        tauri::process::restart(&app.env());
    }
}
