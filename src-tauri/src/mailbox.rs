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
        let service = mdns_service_info(port, handle);
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
    state.server.await.unwrap();
    if let Err(e) = handle
        .state::<ServiceDaemon>()
        .unregister(&state.mdns_fullname)
    {
        log::error!("Failed to unregister MDNS service: {e:?}");
    }

    log::info!("Local mailbox stopped");
    Ok(())
}

const MDNS_SERVICE_TYPE: &str = "_dashchat._tcp.local.";

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

                    // Prefer IPv4, fall back to IPv6 with bracket notation
                    let host = resolved
                        .addresses
                        .iter()
                        .find_map(|addr| match addr {
                            mdns_sd::ScopedIp::V4(ip) => Some(ip.addr().to_string()),
                            _ => None,
                        })
                        .or_else(|| {
                            resolved.addresses.iter().find_map(|addr| match addr {
                                mdns_sd::ScopedIp::V6(ip) => {
                                    Some(format!("[{}]", ip.addr()))
                                }
                                _ => None,
                            })
                        });

                    let Some(host) = host else {
                        log::warn!(
                            "Resolved mdns service {mailbox_id} has no addresses, skipping"
                        );
                        continue;
                    };

                    let n = node.clone();
                    n.mailboxes
                        .register(mailbox_client::toy::ToyMailboxClient::new(
                            mailbox_id.clone(),
                            format!("http://{host}:{port}"),
                        ))
                        .await;
                    log::info!(
                        "*** Added new local mailbox client via mdns: {mailbox_id} ({host}:{port}) ***",
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
