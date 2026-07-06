use crate::app_node::AppNode;
use mailbox_local_server::LocalMailboxServer;
use mailbox_server::BlobSync;
use mdns_sd::ServiceDaemon;
use tauri::{AppHandle, Manager, Runtime};
use tokio::sync::Mutex;

use crate::filesystem::FileSystem;

pub(crate) type LocalMailboxMutex = Mutex<Option<LocalMailboxServer>>;

pub async fn start_local_mailbox<R: Runtime>(handle: &AppHandle<R>) -> anyhow::Result<()> {
    let mutex = handle.state::<LocalMailboxMutex>();
    let mut guard = mutex.lock().await;
    if guard.is_some() {
        return Ok(());
    }

    let node = handle
        .state::<AppNode>()
        .get()
        .await
        .map_err(|e| anyhow::anyhow!(e))?;
    let endpoint = node.iroh_endpoint().await?;
    let path = FileSystem::new(handle)?.local_mailbox_db_path();
    let daemon: ServiceDaemon = handle.state::<ServiceDaemon>().inner().clone();

    let (peer_addr_tx, mut peer_addr_rx) = tokio::sync::mpsc::unbounded_channel();
    let node_for_peer_addrs = node.clone();
    tokio::spawn(async move {
        while let Some(addr) = peer_addr_rx.recv().await {
            if let Err(err) = node_for_peer_addrs.insert_peer_addr(addr).await {
                log::warn!("Failed to register peer addr: {err}");
            }
        }
    });

    // The in-process mailbox shares the node's iroh endpoint and blob store, so
    // its EndpointId equals the node's device id and relayed blobs are served
    // from the same store on the same endpoint. The mDNS instance name therefore
    // encodes that EndpointId and resolves to this shared endpoint. Bind an
    // ephemeral dual-stack port so peers reach us over IPv4 or IPv6.
    let blob_sync = BlobSync::shared(node.blobs(), node.blob_downloader(), endpoint, peer_addr_tx);
    let server = LocalMailboxServer::spawn(
        path,
        "[::]:0",
        Some(blob_sync),
        daemon,
        super::MDNS_SERVICE_TYPE.to_string(),
    )
    .await?;

    *guard = Some(server);

    log::info!("Started local mailbox");

    Ok(())
}

pub async fn stop_local_mailbox<R: Runtime>(handle: &AppHandle<R>) -> anyhow::Result<()> {
    let mutex = handle.state::<LocalMailboxMutex>();
    let mut guard = mutex.lock().await;
    let Some(server) = guard.take() else {
        log::warn!("Tried to stop local mailbox, but it was not running");
        return Ok(());
    };
    log::info!("Sending stop signal to local mailbox...");
    server.stop().await;
    log::info!("Local mailbox stopped");
    Ok(())
}

/// Start/stop the mailbox server, persist the setting, toggle OS autostart,
/// sync the app menu checkbox, and update the tray/badge.
pub async fn set_local_mailbox_server_enabled<R: Runtime>(
    handle: &AppHandle<R>,
    enabled: bool,
) -> anyhow::Result<()> {
    use tauri_plugin_autostart::ManagerExt;

    // Start/stop first — only persist if the operation succeeds.
    if enabled {
        start_local_mailbox(handle).await?;
        crate::tray::show_tray(handle)?;
        set_dock_badge(handle, true);
    } else {
        crate::tray::hide_tray::<R>(handle)?;
        set_dock_badge(handle, false);
        stop_local_mailbox(handle).await?;
    }

    crate::settings::save_mailbox_enabled(handle, enabled);

    // The autostart plugin is only registered in release builds.
    // Log failures instead of propagating — autostart is a convenience
    // feature and shouldn't block the mailbox from working.
    if !tauri::is_dev() {
        let autostart = handle.autolaunch();
        let result = if enabled {
            autostart.enable()
        } else {
            autostart.disable()
        };
        if let Err(err) = result {
            log::error!("Failed to toggle autostart: {err:?}");
        }
    }

    // Keep the app menu's checkbox in sync.
    crate::menu::set_mailbox_toggle_checked(handle, enabled);

    Ok(())
}

/// Show or clear a badge on the dock/taskbar icon to indicate the mailbox is running.
/// The badge is app-level (macOS dock / Linux taskbar) but Tauri only exposes it
/// through a Window, so we grab any available window as the access point.
fn set_dock_badge<R: Runtime>(handle: &AppHandle<R>, active: bool) {
    let window = handle
        .get_webview_window("main")
        .or_else(|| handle.webview_windows().into_values().next());
    if let Some(window) = window {
        let count = if active { Some(1) } else { None };
        if let Err(err) = window.set_badge_count(count) {
            log::warn!("Failed to set dock badge: {err:?}");
        }
    }
}
