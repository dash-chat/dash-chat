use dashchat_node::Node;
#[cfg(not(mobile))]
use tauri::Manager;
use tauri::{AppHandle, State};

#[tauri::command]
pub async fn delete_account(app: AppHandle, node: State<'_, Node>) -> Result<(), String> {
    log::info!("Deleting account...");
    node.delete_account().await.map_err(|e| {
        log::error!("Failed to delete account: {e}");
        e.to_string()
    })?;
    log::info!("Account deleted successfully, restarting app");

    #[cfg(mobile)]
    {
        app.exit(0);
    }

    #[cfg(not(mobile))]
    {
        tauri::process::restart(&app.env());
    }

    Ok(())
}
