use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use clap::Parser;
use futures::FutureExt;
use mdns_sd::{ServiceDaemon, ServiceInfo};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

const MDNS_SERVICE_TYPE: &str = "_dashchat._tcp.local.";

#[derive(Parser, Debug)]
#[command(name = "mailbox-local-server")]
#[command(about = "A standalone LAN mailbox server announced over mDNS so Dash Chat peers on the same network can discover and sync against it", long_about = None)]
struct Args {
    /// Path to the redb database file
    #[arg(short, long, default_value = "mailbox.redb")]
    db_path: PathBuf,

    /// Address to bind the server to.
    #[arg(short, long, default_value = "[::]:3000")]
    addr: SocketAddr,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "mailbox_local_server=debug,mailbox_server=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let args = Args::parse();

    let mailbox_id = load_or_create_mailbox_id(&args.db_path)?;

    let daemon = ServiceDaemon::new()?;
    let mdns_fullname = register_mdns_with_retry(&daemon, &mailbox_id, args.addr.port(), 3)?;

    let signal = tokio::signal::ctrl_c().map(|f| f.expect("failed to listen for event"));
    let result = mailbox_server::spawn_server(args.db_path, args.addr.to_string(), None, signal)
        .await
        .map_err(|e| anyhow::anyhow!("server failed: {e}"));

    if let Err(e) = daemon.unregister(&mdns_fullname) {
        tracing::error!("Failed to unregister MDNS service: {e:?}");
    }
    if let Err(e) = daemon.shutdown() {
        tracing::error!("Failed to shut down MDNS daemon: {e:?}");
    }

    result
}

/// Load the mailbox id from `<db_path>.id`, generating one on first run. Peers
/// key their per-mailbox sync state by this id, so it has to stay stable across
/// restarts.
fn load_or_create_mailbox_id(db_path: &Path) -> anyhow::Result<String> {
    let path = db_path.with_extension("id");
    if path.exists() {
        return Ok(std::fs::read_to_string(&path)?.trim().to_string());
    }

    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    let id = uuid::Uuid::new_v4().simple().to_string();
    std::fs::write(&path, &id)?;
    Ok(id)
}

/// Register the mailbox as an mDNS service, retrying up to `attempts` times.
/// Returns the registered service fullname (needed later to unregister).
fn register_mdns_with_retry(
    daemon: &ServiceDaemon,
    mailbox_id: &str,
    port: u16,
    attempts: u32,
) -> anyhow::Result<String> {
    let mut last_err = None;
    for attempt in 1..=attempts {
        let service = mdns_service_info(mailbox_id, port)?;
        let fullname = service.get_fullname().to_string();
        tracing::info!(
            "Registering local mailbox service via mdns: {} ({})",
            fullname,
            service.get_type()
        );
        match daemon.register(service) {
            Ok(()) => return Ok(fullname),
            Err(e) => {
                tracing::error!(
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

fn mdns_service_info(mailbox_id: &str, port: u16) -> anyhow::Result<ServiceInfo> {
    // Per-server hostname so the A/AAAA owner-name doesn't collide with every
    // other Dash Chat instance on the LAN. A shared hostname can cause one
    // instance's address cache entry to overwrite another's in the resolver.
    let host_name = format!("{mailbox_id}.local.");

    Ok(
        ServiceInfo::new(MDNS_SERVICE_TYPE, mailbox_id, &host_name, "", port, vec![])?
            .enable_addr_auto(),
    )
}
