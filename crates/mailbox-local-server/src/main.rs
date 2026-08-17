use std::path::PathBuf;

use clap::Parser;
use futures::FutureExt;
use mailbox_local_server::MDNS_SERVICE_TYPE;
use mdns_sd::ServiceDaemon;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Parser, Debug)]
#[command(name = "mailbox-local-server")]
#[command(about = "A standalone LAN mailbox server announced over mDNS so Dash Chat peers on the same network can discover and sync against it", long_about = None)]
struct Args {
    /// Path to the redb database file
    #[arg(short, long, default_value = "local-mailbox.redb")]
    db_path: PathBuf,

    /// Port to bind the server to
    #[arg(short, long, default_value_t = 3000)]
    port: u16,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                "mailbox_local_server=debug,mailbox_server=debug,dashchat_utils=debug,iroh=info,iroh_blobs=info".into()
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let args = Args::parse();

    // Load the identity up front (spawn_server reloads the same key from the
    // same db) so the mDNS instance name can encode the EndpointId. The db is
    // dropped again before spawn_server reopens it.
    let endpoint_id = {
        let db = mailbox_server::init_db(args.db_path.clone())
            .map_err(|e| anyhow::anyhow!("failed to open db: {e}"))?;
        mailbox_server::load_or_create_secret_key(&db)
            .map_err(anyhow::Error::msg)?
            .public()
    };

    let daemon = ServiceDaemon::new()?;
    let mdns_fullname = mailbox_local_server::register_mdns_with_retry(
        &daemon,
        MDNS_SERVICE_TYPE,
        endpoint_id,
        args.port,
        3,
    )?;

    let signal = tokio::signal::ctrl_c().map(|f| f.expect("failed to listen for event"));
    // Bind dual-stack (see spawn_local_mailbox_server) so peers can reach us
    // over both the IPv4 and IPv6 addresses the mDNS record auto-announces.
    let addr = format!("[::]:{}", args.port);
    let result = mailbox_server::spawn_server(args.db_path, addr, None, None, None, signal)
        .await
        .map_err(|e| anyhow::anyhow!("server failed: {e}"));

    if let Err(e) = daemon.unregister(&mdns_fullname) {
        log::error!("Failed to unregister MDNS service: {e:?}");
    }
    if let Err(e) = daemon.shutdown() {
        log::error!("Failed to shut down MDNS daemon: {e:?}");
    }

    result
}
