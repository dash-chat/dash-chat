use dashchat_node::Node;
#[cfg(not(mobile))]
use tauri::Manager;
use tauri::{AppHandle, State};

#[tauri::command]
pub fn delete_account(app: AppHandle, node: State<'_, Node>) {
    node.delete_account();

    #[cfg(mobile)]
    {
        app.exit(0);
    }

    #[cfg(not(mobile))]
    {
        tauri::process::restart(&app.env());
    }
}
