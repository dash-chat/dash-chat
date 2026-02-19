use futures::FutureExt;
use mdns_sd::{ServiceDaemon, ServiceInfo};
use tauri::{AppHandle, Manager, Runtime};
use tokio::sync::Mutex;

use crate::filesystem::FileSystem;

pub(crate) struct LocalMailboxState {
    stop_signal: tokio::sync::oneshot::Sender<()>,
    server: tokio::task::JoinHandle<()>,
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

    let mut result = Ok(());
    let mut port = 0;
    for attempt in 1..=3 {
        port = free_port()?;
        let service = mdns_service_info(port, handle);
        log::info!(
            "Registering local mailbox service via mdns: {} ({})",
            service.get_fullname(),
            service.get_type()
        );

        match handle.state::<ServiceDaemon>().register(service) {
            Ok(()) => {
                break;
            }
            Err(e) => {
                log::error!("Failed to register local mailbox service via mdns, attempt {attempt} of 3, error: {e:?}");
                result = Err(e);
            }
        }
    }
    result?;

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
    state.server.await.unwrap();
    if let Err(e) = handle
        .state::<ServiceDaemon>()
        .unregister(MDNS_SERVICE_TYPE)
    {
        log::error!("Failed to unregister MDNS service: {e:?}");
    }

    log::info!("Local mailbox stopped");
    Ok(())
}

const MDNS_SERVICE_TYPE: &str = "_dashchat._udp.local.";

pub fn spawn_local_mailbox_mdns_discovery<R: Runtime>(
    handle: &AppHandle<R>,
    node: dashchat_node::Node,
) -> anyhow::Result<()> {
    let mdns = handle.state::<ServiceDaemon>();
    let receiver = mdns.browse(MDNS_SERVICE_TYPE)?;

    tokio::spawn(async move {
        while let Ok(event) = receiver.recv() {
            match event {
                mdns_sd::ServiceEvent::ServiceResolved(resolved) => {
                    let mailbox_id = resolved.fullname;
                    let port = resolved.port;
                    let ip = resolved
                        .addresses
                        .iter()
                        .find_map(|addr| match addr {
                            mdns_sd::ScopedIp::V4(ip) => Some(ip.addr().to_string()),
                            _ => None,
                        })
                        .unwrap_or_default();
                    let n = node.clone();
                    let ip2 = ip.clone();
                    n.mailboxes
                        .register(mailbox_client::toy::ToyMailboxClient::new(
                            mailbox_id.clone(),
                            format!("http://{ip2}:{port}",),
                        ))
                        .await;
                    log::info!(
                        "*** Added new local mailbox client via mdns: {mailbox_id} ({ip2}:{port}) ***",
                    );
                }
                other_event => {
                    log::trace!("((( Received other mdns event: {:?} )))", &other_event);
                }
            }
        }

        log::warn!("mdns discovery loop ended");
    });

    Ok(())
}

fn mdns_service_info<R: Runtime>(port: u16, _handle: &AppHandle<R>) -> ServiceInfo {
    let instance_name = nanoid::nanoid!(7);

    let host_name = "0.0.0.0.local.";

    ServiceInfo::new(
        MDNS_SERVICE_TYPE,
        &instance_name,
        host_name,
        "",
        port,
        vec![],
    )
    .unwrap()
    .enable_addr_auto()
}

fn free_port() -> anyhow::Result<u16> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    Ok(listener.local_addr()?.port())
}
