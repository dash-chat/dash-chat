use futures::FutureExt;
use mdns_sd::{ServiceDaemon, ServiceInfo};
use tauri::{AppHandle, Manager, Runtime};
use tokio::sync::Mutex;

use crate::filesystem::FileSystem;

pub(crate) struct LocalMailboxState {
    stop_signal: tokio::sync::oneshot::Sender<()>,
    server: tokio::task::JoinHandle<()>,
    mdns_fullname: String,
}

pub(crate) type LocalMailboxMutex = Mutex<Option<LocalMailboxState>>;

pub async fn start_local_mailbox<R: Runtime>(handle: &AppHandle<R>) -> anyhow::Result<()> {
    let mutex = handle.state::<LocalMailboxMutex>();
    let mut guard = mutex.lock().await;
    if guard.is_some() {
        return Ok(());
    }

    let (stop_signal, stop_signal_rx) = tokio::sync::oneshot::channel();
    let stop_signal_rx = stop_signal_rx.map(|f| f.expect("failed to listen for event"));
    let path = FileSystem::new(handle).local_mailbox_db_path()?;

    let mut last_err = None;
    let mut port = 0;
    let mut mdns_fullname = String::new();
    for attempt in 1..=3 {
        port = free_port()?;
        let service = mdns_service_info(port, handle)?;
        let fullname = service.get_fullname().to_string();
        log::info!(
            "Registering local mailbox service via mdns: {} ({})",
            fullname,
            service.get_type()
        );

        match handle.state::<ServiceDaemon>().register(service) {
            Ok(()) => {
                mdns_fullname = fullname;
                last_err = None;
                break;
            }
            Err(e) => {
                log::error!("Failed to register local mailbox service via mdns, attempt {attempt} of 3, error: {e:?}");
                last_err = Some(e);
            }
        }
    }
    if let Some(e) = last_err {
        return Err(e.into());
    }

    let addr = format!("0.0.0.0:{port}");
    let server = tokio::spawn(async move {
        match mailbox_server::spawn_server(path, addr, stop_signal_rx).await {
            Ok(_) => (),
            Err(e) => log::error!("Failed to start local mailbox: {e:?}"),
        }
    });

    *guard = Some(LocalMailboxState {
        server,
        stop_signal,
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
    if !tauri::is_dev() {
        let autostart = handle.autolaunch();
        if enabled {
            autostart.enable()?;
        } else {
            autostart.disable()?;
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

fn mdns_service_info<R: Runtime>(port: u16, _handle: &AppHandle<R>) -> anyhow::Result<ServiceInfo> {
    let instance_name = nanoid::nanoid!(7);

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
