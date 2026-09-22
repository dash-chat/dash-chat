use std::path::PathBuf;

use clap::Parser;
use futures::FutureExt;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Parser, Debug)]
#[command(name = "mailbox-local-server")]
#[command(about = "A standalone LAN mailbox server announced over mDNS so Dash Chat peers on the same network can discover and sync against it", long_about = None)]
struct Args {
    /// Path to the redb database file
    #[arg(short, long, default_value = "mailbox.redb")]
    db_path: PathBuf,

    /// Port to listen on, on every interface.
    #[arg(short, long, default_value_t = 3000)]
    port: u16,

    /// P2P network id, as 64 hex characters (defaults to the production network)
    #[arg(long, value_parser = mailbox_server::parse_network_id)]
    network_id: Option<[u8; 32]>,
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

    // Load the identity up front so the mDNS instance name can encode the
    // EndpointId; spawn_server reloads the same key from the same db.
    let endpoint_id = {
        let db = mailbox_server::init_db(args.db_path.clone())
            .map_err(|e| anyhow::anyhow!("failed to open db: {e}"))?;
        mailbox_server::load_or_create_secret_key(&db)
            .map_err(anyhow::Error::msg)?
            .public()
    };

    // Bound before anything is announced, so a port that cannot be served is
    // never advertised.
    let listener = tokio::net::TcpListener::bind(format!("[::]:{}", args.port)).await?;
    let announcement = mailbox_local_server::spawn_local_hub_announcement(endpoint_id, args.port)?;

    let signal = tokio::signal::ctrl_c().map(|f| f.expect("failed to listen for event"));
    // No relay — the server stays fully local.
    let served = mailbox_server::spawn_server(
        args.db_path,
        listener,
        None,
        None,
        None,
        args.network_id.unwrap_or(*dashchat_utils::NETWORK_ID),
        signal,
    )
    .await;

    announcement.shutdown().await;
    served.map_err(|e| anyhow::anyhow!("server failed: {e}"))
}
