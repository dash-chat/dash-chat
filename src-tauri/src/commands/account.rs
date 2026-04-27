use dashchat_node::{node::ShutdownError, Node};
#[cfg(not(mobile))]
use tauri::Manager;
use tauri::{AppHandle, State};

const PENDING_DELETE_MARKER: &str = ".pending-delete";

/// Check for a pending-delete marker and remove the data directory if found.
/// Must be called before opening any databases.
pub fn delete_account_if_pending(data_path: &std::path::Path) {
    if data_path.join(PENDING_DELETE_MARKER).exists() {
        log::info!("Pending account deletion detected, removing data directory...");
        if let Err(e) = std::fs::remove_dir_all(data_path) {
            log::error!("Failed to remove data directory during pending delete: {e}");
        } else {
            log::info!("Data directory removed successfully");
        }
    }
}

#[tauri::command]
pub async fn delete_account(app: AppHandle, node: State<'_, Node>) -> Result<(), String> {
    log::info!("Deleting account...");

    // If we fail to shutdown, we still continue to delete all data and restart the app
    match node.shutdown().await {
        Ok(()) => {}
        Err(ShutdownError::WaitOnDatabaseHandlesError(err)) => {
            log::error!(
                "Failed to wait on database handles closing, continuing to delete account: {err}"
            );
        }
    }

    #[cfg(mobile)]
    {
        // On mobile (POSIX), open file handles don't block deletion
        std::fs::remove_dir_all(node.data_path()).map_err(|e| {
            log::error!("Failed to delete account: {e}");
            String::from("Failed to delete account.")
        })?;
        log::info!("Account deleted successfully");
        app.exit(0);
        return Ok(());
    }

    #[cfg(not(mobile))]
    {
        // On desktop, database file handles may still be open (Windows locks open files).
        // Write a marker so the data directory is deleted on next startup before
        // any databases are opened.
        let marker_path = node.data_path().join(PENDING_DELETE_MARKER);
        std::fs::write(&marker_path, b"").map_err(|e| {
            log::error!("Failed to write pending-delete marker: {e}");
            String::from("Failed to delete account.")
        })?;
        log::info!("Pending-delete marker written, restarting app");
        tauri::process::restart(&app.env());
    }
}
