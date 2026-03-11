use dashchat_node::Node;
#[cfg(not(mobile))]
use tauri::Manager;
use tauri::{AppHandle, State};

#[tauri::command]
pub async fn delete_account(app: AppHandle, node: State<'_, Node>) -> Result<(), String> {
    node.delete_account().await.map_err(|e| e.to_string())?;

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
