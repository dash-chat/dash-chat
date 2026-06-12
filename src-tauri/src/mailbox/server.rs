use std::sync::{Arc, Mutex as StdMutex};

use dashchat_node::{DeviceId, Node};
use futures::FutureExt;
use mdns_sd::{ServiceDaemon, ServiceInfo};
use tauri::{AppHandle, Manager, Runtime};
use tokio::sync::Mutex;

use crate::filesystem::FileSystem;

pub(crate) struct LocalMailboxState {
    stop_signal: tokio::sync::oneshot::Sender<()>,
    server: tokio::task::JoinHandle<()>,
    /// Currently-registered mDNS fullname. Owned by `run_interface_watcher`
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

    let device_id = handle.state::<Node>().device_id();

    let (stop_signal, stop_signal_rx) = tokio::sync::oneshot::channel();
    let stop_signal_rx = stop_signal_rx.map(|f| f.expect("failed to listen for event"));
    let path = FileSystem::new(handle)?.local_mailbox_db_path();

    let daemon: ServiceDaemon = handle.state::<ServiceDaemon>().inner().clone();
    let port = free_port()?;
    let mdns_fullname = Arc::new(StdMutex::new(register_mdns_with_retry(&daemon, port, &device_id, 3)?));

    // Bind dual-stack so peers can reach us over both the IPv4 and IPv6
    // addresses the mDNS record auto-announces. A `::` socket accepts IPv4
    // connections as v4-mapped addresses on platforms where `IPV6_V6ONLY`
    // defaults off (macOS, Linux).
    let addr = format!("[::]:{port}");
    let server = tokio::spawn(async move {
        match mailbox_server::spawn_server(path, addr, None, stop_signal_rx).await {
            Ok(_) => (),
            Err(e) => log::error!("Failed to start local mailbox: {e:?}"),
        }
    });

    let (interface_watcher_stop, interface_watcher_stop_rx) = tokio::sync::oneshot::channel();
    let watcher_daemon = daemon.clone();
    let watcher_fullname = mdns_fullname.clone();
    let watcher_device_id = device_id.clone();
    let interface_watcher = tokio::spawn(async move {
        run_interface_watcher(
            watcher_daemon,
            watcher_device_id,
            port,
            watcher_fullname,
            interface_watcher_stop_rx,
        )
        .await;
    });

    *guard = Some(LocalMailboxState {
        server,
        stop_signal,
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
    let _ = state.stop_signal.send(());
    if let Err(err) = state.server.await {
        log::error!("Local mailbox server task ended unexpectedly: {err}");
    }
    let fullname = state.mdns_fullname.lock().unwrap_or_else(|p| p.into_inner()).clone();
    if let Err(e) = handle.state::<ServiceDaemon>().unregister(&fullname) {
        log::error!("Failed to unregister MDNS service: {e:?}");
    }

    log::info!("Local mailbox stopped");
    Ok(())
}

/// Start/stop the mailbox server, persist the setting, toggle OS autostart,
/// sync the app menu checkbox, and update the tray/badge.
pub async fn set_local_mailbox_server_enabled<R: Runtime>(handle: &AppHandle<R>, enabled: bool) -> anyhow::Result<()> {
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

fn free_port() -> anyhow::Result<u16> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    Ok(listener.local_addr()?.port())
}

fn register_mdns_with_retry(
    daemon: &ServiceDaemon,
    port: u16,
    device_id: &DeviceId,
    attempts: u32,
) -> anyhow::Result<String> {
    let mut last_err = None;
    for attempt in 1..=attempts {
        let service = mdns_service_info(port, device_id)?;
        let fullname = service.get_fullname().to_string();
        log::info!(
            "Registering local mailbox service via mdns: {} ({})",
            fullname,
            service.get_type()
        );
        match daemon.register(service) {
            Ok(()) => return Ok(fullname),
            Err(e) => {
                log::error!(
                    "Failed to register local mailbox service via mdns, attempt {attempt} of {attempts}, error: {e:?}"
                );
                last_err = Some(e);
            }
        }
    }
    Err(last_err
        .map(anyhow::Error::from)
        .unwrap_or_else(|| anyhow::anyhow!("failed to register local mailbox service via mdns")))
}

/// Re-announce the mDNS record whenever a network interface comes up or down.
/// `enable_addr_auto()` only enumerates interfaces at registration time, so without
/// this the announcement misses interfaces (like macOS Internet Sharing's bridge100)
/// that appear after the mailbox starts. Returns when the stop signal fires or the
/// watcher stream ends.
async fn run_interface_watcher(
    daemon: ServiceDaemon,
    device_id: DeviceId,
    port: u16,
    fullname: Arc<StdMutex<String>>,
    mut stop: tokio::sync::oneshot::Receiver<()>,
) {
    use futures::StreamExt;
    let mut watcher = match if_watch::tokio::IfWatcher::new() {
        Ok(w) => w,
        Err(err) => {
            log::warn!("Failed to start mailbox interface watcher: {err:?}");
            return;
        }
    };

    loop {
        tokio::select! {
            biased;
            _ = &mut stop => {
                log::debug!("Mailbox interface watcher stopped");
                return;
            }
            event = watcher.next() => {
                let Some(event) = event else {
                    log::warn!("Mailbox interface watcher stream ended");
                    return;
                };
                match event {
                    Ok(if_watch::IfEvent::Up(net)) => {
                        log::info!("Mailbox interface watcher: up {net}, re-announcing mDNS");
                    }
                    Ok(if_watch::IfEvent::Down(net)) => {
                        log::info!("Mailbox interface watcher: down {net}, re-announcing mDNS");
                    }
                    Err(err) => {
                        log::warn!("Mailbox interface watcher error: {err:?}");
                        continue;
                    }
                }

                if !debounce_reannounce_burst(&mut watcher, &mut stop).await {
                    return;
                }
                let prev = fullname.lock().unwrap_or_else(|p| p.into_inner()).clone();
                if let Some(new_fullname) = reannounce_mdns(&daemon, &prev, port, &device_id) {
                    *fullname.lock().unwrap_or_else(|p| p.into_inner()) = new_fullname;
                }
            }
        }
    }
}

/// Wait 500ms after an interface event so a burst of related changes triggers
/// a single re-announce. Returns false if the stop signal fired during the
/// wait (caller should treat that as shutdown) or if the watcher stream ended.
async fn debounce_reannounce_burst(
    watcher: &mut if_watch::tokio::IfWatcher,
    stop: &mut tokio::sync::oneshot::Receiver<()>,
) -> bool {
    use futures::StreamExt;
    let debounce = tokio::time::sleep(std::time::Duration::from_millis(500));
    tokio::pin!(debounce);
    loop {
        tokio::select! {
            biased;
            _ = &mut *stop => return false,
            _ = &mut debounce => return true,
            next = watcher.next() => {
                if next.is_none() {
                    return false;
                }
            }
        }
    }
}

/// Returns the fullname that is now registered with the daemon, or `None` if
/// the re-register failed (in which case the previous `fullname` is no longer
/// registered either — the unregister already ran).
fn reannounce_mdns(daemon: &ServiceDaemon, fullname: &str, port: u16, device_id: &DeviceId) -> Option<String> {
    let _ = daemon.unregister(fullname);
    let service = match mdns_service_info(port, device_id) {
        Ok(service) => service,
        Err(err) => {
            log::warn!("Failed to build mDNS service info for re-announce: {err:?}");
            return None;
        }
    };
    let new_fullname = service.get_fullname().to_string();
    if let Err(err) = daemon.register(service) {
        log::warn!("Failed to re-register mailbox mDNS: {err:?}");
        return None;
    }
    Some(new_fullname)
}

fn mdns_service_info(port: u16, device_id: &DeviceId) -> anyhow::Result<ServiceInfo> {
    // Derive a stable instance name from the device id so peers can keep using
    // the same MailboxId across restarts. The instance name lives in a single
    // DNS label (63-byte limit); 32 hex chars (16 bytes of public key) is plenty
    // for collision resistance on a local network.
    let mut instance_name = device_id.to_string();
    instance_name.truncate(32);

    // Per-device hostname so the A/AAAA owner-name doesn't collide with every
    // other Dash Chat instance on the LAN. A shared hostname can cause one
    // instance's address cache entry to overwrite another's in the resolver.
    let host_name = format!("{instance_name}.local.");

    Ok(ServiceInfo::new(super::MDNS_SERVICE_TYPE, &instance_name, &host_name, "", port, vec![])?.enable_addr_auto())
}
