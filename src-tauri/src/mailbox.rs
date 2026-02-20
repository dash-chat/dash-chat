use mdns_sd::{ServiceDaemon, ServiceInfo};
use tauri::{AppHandle, Manager, Runtime};

const MDNS_SERVICE_TYPE: &str = "_dashchat._tcp.local.";

#[cfg(not(mobile))]
pub mod server;

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
