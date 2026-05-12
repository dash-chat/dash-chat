use std::time::Duration;

use dashchat_node::{DeviceId, Node};
use futures::FutureExt;
use mdns_sd::{ServiceDaemon, ServiceInfo};
use tauri::{AppHandle, Manager, Runtime};
use tokio::sync::Mutex;

use crate::filesystem::FileSystem;

/// How often to re-announce the mDNS service so peers that join the network
/// (or open the app) after the desktop's initial announcement still discover it.
const MDNS_REANNOUNCE_INTERVAL: Duration = Duration::from_secs(30);

pub(crate) struct LocalMailboxState {
    stop_signal: tokio::sync::oneshot::Sender<()>,
    server: tokio::task::JoinHandle<()>,
    reannounce: tokio::task::JoinHandle<()>,
    mdns_fullname: String,
}

pub(crate) type LocalMailboxMutex = Mutex<Option<LocalMailboxState>>;

pub async fn start_local_mailbox<R: Runtime>(handle: &AppHandle<R>) -> anyhow::Result<()> {
    let mutex = handle.state::<LocalMailboxMutex>();
    let mut guard = mutex.lock().await;
    if guard.is_some() {
        return Ok(());
    }

    let device_id = handle.state::<Node>().device_id();

    let (stop_signal, stop_signal_rx) = tokio::sync::oneshot::channel();
    let stop_signal_rx = stop_signal_rx.map(|f| f.expect("failed to listen for event"));
    let path = FileSystem::new(handle)?.local_mailbox_db_path();

    let daemon = handle.state::<ServiceDaemon>().inner().clone();
    let port = free_port()?;
    let service = mdns_service_info(port, &device_id)?;
    log::info!(
        "Registering local mailbox service via mdns: {} ({})",
        service.get_fullname(),
        service.get_type()
    );

    let mdns_fullname = service.get_fullname().to_string();
    let addr = format!("0.0.0.0:{}", service.get_port());
    let server = tokio::spawn(async move {
        match mailbox_server::spawn_server(path, addr, None, stop_signal_rx).await {
            Ok(_) => (),
            Err(e) => log::error!("Failed to start local mailbox: {e:?}"),
        }
    });

    // Register immediately on the first tick and then periodically re-announce so
    // peers that come online later still discover us. mdns_sd's daemon only sends
    // two unsolicited announcements at startup (RFC 6762 §8.3) and otherwise
    // relies on responding to queries — if a late-joining client's query response
    // is lost, we'd be invisible until the next query, which can be minutes out
    // under exponential backoff. Using the periodic loop for the initial register
    // also gives us a free retry on transient failures.
    let reannounce = tokio::spawn(async move {
        let mut tick = tokio::time::interval(MDNS_REANNOUNCE_INTERVAL);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tick.tick().await;
            match daemon.register(service.clone()) {
                Ok(()) => log::debug!("Announced local mailbox via mdns"),
                Err(e) => log::warn!("Failed to announce local mailbox via mdns: {e:?}"),
            }
        }
    });

    *guard = Some(LocalMailboxState {
        server,
        stop_signal,
        reannounce,
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
    state.reannounce.abort();
    let _ = state.stop_signal.send(());
    state.server.await?;
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
    sync_menu_toggle(handle, enabled);

    Ok(())
}

/// Update the "toggle-local-mailbox" CheckMenuItem in the app menu, if present.
fn sync_menu_toggle<R: Runtime>(handle: &AppHandle<R>, enabled: bool) {
    let Some(menu) = handle.menu() else { return };
    for item in menu.items().unwrap_or_default() {
        if let Some(submenu) = item.as_submenu() {
            if let Some(toggle) = submenu.get("toggle-local-mailbox") {
                if let Some(check) = toggle.as_check_menuitem() {
                    let _ = check.set_checked(enabled);
                }
            }
        }
    }
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

fn free_port() -> anyhow::Result<u16> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    Ok(listener.local_addr()?.port())
}

fn mdns_service_info(port: u16, device_id: &DeviceId) -> anyhow::Result<ServiceInfo> {
    // Derive a stable instance name from the device id so peers can keep using
    // the same MailboxId across restarts. The instance name lives in a single
    // DNS label (63-byte limit); 32 hex chars (16 bytes of public key) is plenty
    // for collision resistance on a local network.
    let mut instance_name = device_id.to_string();
    instance_name.truncate(32);

    let host_name = "0.0.0.0.local.";

    Ok(ServiceInfo::new(
        super::MDNS_SERVICE_TYPE,
        &instance_name,
        host_name,
        "",
        port,
        vec![],
    )?
    .enable_addr_auto())
}
