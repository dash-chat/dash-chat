use dashchat_node::Node;
#[cfg(not(mobile))]
use tauri::Manager;
use tauri::{AppHandle, State};

#[tauri::command]
pub fn delete_account(app: AppHandle, node: State<'_, Node>) -> Result<(), String> {
    let data_path = node.data_path().clone();
    std::fs::remove_dir_all(&data_path).map_err(|e| e.to_string())?;

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
