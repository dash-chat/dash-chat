use crate::node::AppNodeManager;
use mailbox_local_server::LocalMailboxServer;
use mdns_sd::ServiceDaemon;
use tauri::{AppHandle, Manager, Runtime};
use tokio::sync::Mutex;

use crate::filesystem::FileSystem;

pub(crate) struct LocalMailboxState {
    server: LocalMailboxServer,
    mdns_fullname: String,
}

pub(crate) type LocalMailboxMutex = Mutex<Option<LocalMailboxState>>;

pub async fn start_local_mailbox<R: Runtime>(handle: &AppHandle<R>) -> anyhow::Result<()> {
    let mutex = handle.state::<LocalMailboxMutex>();
    let mut guard = mutex.lock().await;
    if guard.is_some() {
        return Ok(());
    }

    let node = handle
        .state::<AppNodeManager>()
        .get()
        .await
        .map_err(|e| anyhow::anyhow!(e))?;
    let endpoint_id = node.endpoint_id();
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
    // encodes that EndpointId and resolves to this shared endpoint.
    let blob_sync = node.blob_sync_optional().expect("blob sync is enabled");
    let server = mailbox_local_server::spawn_local_mailbox_server(
        path,
        blob_sync.blobs.clone(),
        blob_sync.downloader(),
        endpoint,
        None,
        None,
        peer_addr_tx,
    )
    .await?;

    // Interface changes (a new network appearing after startup) are handled by
    // the mdns-sd daemon itself: it re-checks interfaces periodically and
    // announces `addr_auto` services on new ones. Re-registering the service
    // ourselves must be avoided — mdns-sd 0.20 probes on re-register, mistakes
    // its own just-unregistered records for a conflicting peer, and renames the
    // service, after which it no longer answers SRV refresh queries and
    // browsers drop it when the announcement TTL expires.
    let mdns_fullname = mailbox_local_server::register_mdns_with_retry(
        &daemon,
        super::MDNS_SERVICE_TYPE,
        endpoint_id,
        server.port,
        3,
    )?;

    *guard = Some(LocalMailboxState {
        server,
        mdns_fullname,
    });

    log::info!("Started local mailbox");

    Ok(())
}

pub async fn stop_local_mailbox<R: Runtime>(handle: &AppHandle<R>) -> anyhow::Result<()> {
    let mutex = handle.state::<LocalMailboxMutex>();
    let mut guard = mutex.lock().await;
    let Some(state) = guard.take() else {
        log::warn!("Tried to stop local mailbox, but it was not running");
        return Ok(());
    };
    log::info!("Sending stop signal to local mailbox...");
    state.server.stop().await;
    if let Err(e) = handle
        .state::<ServiceDaemon>()
        .unregister(&state.mdns_fullname)
    {
        log::error!("Failed to unregister MDNS service: {e:?}");
    }

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
