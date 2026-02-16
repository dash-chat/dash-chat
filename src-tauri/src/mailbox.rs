use futures::FutureExt;
use mdns_sd::{ServiceDaemon, ServiceInfo};
use tauri::{AppHandle, Manager, Runtime};
use tokio::sync::Mutex;

pub struct LocalMailboxState {
    stop_signal: tokio::sync::oneshot::Sender<()>,
    server: tokio::task::JoinHandle<()>,
}

pub(crate) type LocalMailboxMutex = Mutex<Option<LocalMailboxState>>;

pub fn start_local_mailbox<R: Runtime>(
    handle: &AppHandle<R>,
) -> Result<(), Box<dyn std::error::Error>> {
    tauri::async_runtime::block_on(async move {
        let state_mutex = handle.state::<LocalMailboxMutex>();
        let mut state = state_mutex.lock().await;
        if state.is_some() {
            return Ok(());
        }

        let (stop_signal_tx, stop_signal_rx) = tokio::sync::oneshot::channel();
        let stop_signal_rx = stop_signal_rx.map(|f| f.expect("failed to listen for event"));
        let path = crate::filesystem::local_data_dir(handle)?.join("local-mailbox.redb");

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

        log::info!("Started local mailbox");
        if state
            .replace(LocalMailboxState {
                stop_signal: stop_signal_tx,
                server,
            })
            .is_some()
        {
            unreachable!("Replaced existing mailbox state with new state, this should not happen.");
        }
        Ok(())
    })
}

pub fn stop_local_mailbox<R: Runtime>(handle: &AppHandle<R>) {
    tauri::async_runtime::block_on(async move {
        let state_mutex = handle.state::<LocalMailboxMutex>();
        let mut state = state_mutex.lock().await;
        let Some(state) = state.take() else {
            log::warn!("Tried to stop local mailbox, but it was not running");
            return;
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
    });
}

const MDNS_SERVICE_TYPE: &str = "_dashchat._udp.local.";

pub fn spawn_local_mailbox_mdns_discovery<R: Runtime>(
    handle: &AppHandle<R>,
    node: dashchat_node::Node,
) -> Result<(), Box<dyn std::error::Error>> {
    let mdns = handle.state::<ServiceDaemon>();
    let receiver = mdns.browse(MDNS_SERVICE_TYPE)?;

    tokio::spawn(async move {
        while let Ok(event) = receiver.recv() {
            match event {
                mdns_sd::ServiceEvent::ServiceResolved(resolved) => {
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
                        .add(mailbox_client::toy::ToyMailboxClient::new(format!(
                            "http://{ip2}:{port}",
                        )))
                        .await;
                    log::info!(
                        "*** Added new local mailbox client via mdns: {} ({ip2}:{port}) ***",
                        resolved.fullname,
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

fn free_port() -> Result<u16, Box<dyn std::error::Error>> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    Ok(listener.local_addr()?.port())
}
