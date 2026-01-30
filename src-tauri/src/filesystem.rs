use std::path::PathBuf;
use tauri::{AppHandle, Manager};

// In production, use the local data dir from the operating system
// In development, use a numbered directory in the local data dir
pub fn local_data_dir(handle: &AppHandle) -> anyhow::Result<PathBuf> {
    let local_data_path = if tauri::is_dev() {
        let mut local_data_path = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR")?);
        local_data_path.pop();
        local_data_path = local_data_path
            .join(".dev-dbs")
            .join(format!("agent-{}", std::env::var("AGENT")?));
        local_data_path
    } else {
        handle.path().local_data_dir()?
    };
    if !local_data_path.exists() {
        std::fs::create_dir_all(&local_data_path)?;
    }
    Ok(local_data_path)
}
