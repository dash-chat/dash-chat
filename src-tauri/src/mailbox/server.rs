use std::sync::{Arc, Mutex as StdMutex};

use dashchat_node::Node;
use mailbox_local_server::LocalMailboxServer;
use mdns_sd::ServiceDaemon;
use tauri::{AppHandle, Manager, Runtime};
use tokio::sync::Mutex;

use crate::filesystem::FileSystem;

pub(crate) struct LocalMailboxState {
    server: LocalMailboxServer,
    /// Currently-registered mDNS fullname. Owned by the interface watcher
    /// which overwrites it on every re-announce so `stop_local_mailbox`
    /// unregisters the actually-registered service rather than a stale one
    /// from a previous re-announce cycle.
    mdns_fullname: Arc<StdMutex<String>>,
    interface_watcher_stop: tokio::sync::oneshot::Sender<()>,
    interface_watcher: tokio::task::JoinHandle<()>,
}

pub(crate) type LocalMailboxMutex = Mutex<Option<LocalMailboxState>>;

pub async fn start_local_mailbox<R: Runtime>(handle: &AppHandle<R>) -> anyhow::Result<()> {
    let mutex = handle.state::<LocalMailboxMutex>();
    let mut guard = mutex.lock().await;
    if guard.is_some() {
        return Ok(());
    }

    let node = handle.state::<Node>();
    let endpoint_id = node.endpoint_id();
    let path = FileSystem::new(handle)?.local_mailbox_db_path();
    let daemon: ServiceDaemon = handle.state::<ServiceDaemon>().inner().clone();

    // The in-process mailbox shares the node's iroh endpoint and blob store, so
    // its EndpointId equals the node's device id and relayed blobs are served
    // from the same store on the same endpoint. The mDNS instance name therefore
    // encodes that EndpointId and resolves to this shared endpoint.
    let server = mailbox_local_server::spawn_local_mailbox_server(
        path,
        node.blobs(),
        node.blob_downloader(),
        endpoint_id,
        None,
    )
    .await?;

    let mdns_fullname = Arc::new(StdMutex::new(
        mailbox_local_server::register_mdns_with_retry(
            &daemon,
            super::MDNS_SERVICE_TYPE,
            endpoint_id,
            server.port,
            3,
        )?,
    ));

    let (interface_watcher_stop, interface_watcher_stop_rx) = tokio::sync::oneshot::channel();
    let interface_watcher = mailbox_local_server::spawn_interface_watcher(
        daemon,
        super::MDNS_SERVICE_TYPE.to_string(),
        endpoint_id,
        server.port,
        mdns_fullname.clone(),
        interface_watcher_stop_rx,
    );

    *guard = Some(LocalMailboxState {
        server,
        mdns_fullname,
        interface_watcher_stop,
        interface_watcher,
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
    let _ = state.interface_watcher_stop.send(());
    if let Err(err) = state.interface_watcher.await {
        log::error!("Mailbox interface watcher ended unexpectedly: {err}");
    }
    state.server.stop().await;
    let fullname = state
        .mdns_fullname
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clone();
    if let Err(e) = handle.state::<ServiceDaemon>().unregister(&fullname) {
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
