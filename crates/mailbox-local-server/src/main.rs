use std::path::PathBuf;

use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Parser, Debug)]
#[command(name = "mailbox-local-server")]
#[command(about = "A standalone LAN mailbox server announced over mDNS so Dash Chat peers on the same network can discover and sync against it", long_about = None)]
struct Args {
    /// Path to the redb database file
    #[arg(short, long, default_value = "mailbox.redb")]
    db_path: PathBuf,

    /// Port to listen on, on every interface. Without it, the port this hub
    /// served on last is reused when still free, else any free port is taken.
    #[arg(short, long)]
    port: Option<u16>,

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
    let listener = match args.port {
        Some(port) => tokio::net::TcpListener::bind(format!("[::]:{port}")).await?,
        None => {
            let (listener, _) = mailbox_local_server::reserve_listener(&args.db_path)?;
            listener.set_nonblocking(true)?;
            tokio::net::TcpListener::from_std(listener)?
        }
    };
    let port = listener.local_addr()?.port();
    let announcement = std::sync::Arc::new(mailbox_local_server::spawn_local_hub_announcement(
        endpoint_id,
        port,
    )?);

    // The goodbye goes out while the server is still serving: a browser that
    // hears it retires the hub at once, where the refused probes it would get
    // from a stopped one deliberately mean nothing.
    let signal = {
        let announcement = announcement.clone();
        async move {
            tokio::signal::ctrl_c()
                .await
                .expect("failed to listen for event");
            announcement.shutdown().await;
        }
    };
    // No relay — the server stays fully local.
    let served = mailbox_server::spawn_server(
        args.db_path,
        listener,
        None,
        mailbox_server::MailboxBlobs::Own {
            relay_url: None,
            network_id: args.network_id.unwrap_or(*dashchat_utils::NETWORK_ID),
        },
        signal,
    )
    .await;

    // However the server stopped: on ctrl-c this already said it, on any other
    // exit it is late but still better than leaving peers to the lapse.
    announcement.shutdown().await;
    served.map_err(|e| anyhow::anyhow!("server failed: {e}"))
}
