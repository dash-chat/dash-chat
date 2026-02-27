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
    crate::tray::show_tray(handle)?;

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
    crate::tray::hide_tray::<R>(handle)?;

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
