use std::path::PathBuf;
use tauri::{AppHandle, Manager};

// In production, use the local data dir from the operating system.
// In desktop development, use DEV_DBS_PATH/agent-{AGENT} (set in mprocs.yaml) so multiple
// agents can run side-by-side. On mobile development we fall through to the OS
// data dir because DEV_DBS_PATH points to the build machine, not the device.
pub fn local_data_dir(handle: &AppHandle) -> anyhow::Result<PathBuf> {
    let local_data_path = if cfg!(mobile) || !tauri::is_dev() {
        handle.path().local_data_dir()?
    } else {
        let base = PathBuf::from(std::env::var("DEV_DBS_PATH")?);
        base.join(format!("agent-{}", std::env::var("AGENT")?))
    };
    if !local_data_path.exists() {
        std::fs::create_dir_all(&local_data_path)?;
    }
    Ok(local_data_path)
}
