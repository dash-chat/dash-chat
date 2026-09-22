use clap::Parser;
use dashchat_utils::{NETWORK_ID, RELAY_URL};
use futures::FutureExt;
use mailbox_server::spawn_server;
use p2panda_net::NetworkId;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Parser, Debug)]
#[command(name = "mailbox_server")]
#[command(about = "A simple mailbox server for storing and retrieving messages", long_about = None)]
struct Args {
    /// Path to the redb database file
    #[arg(short, long, default_value = "mailbox.redb")]
    db_path: String,

    /// Address to bind the server to
    #[arg(short, long, default_value = "0.0.0.0:3000")]
    addr: String,

    /// URL of the push notifications server (enables push notification integration)
    #[arg(long)]
    push_notifications_url: Option<String>,

    /// P2P network id, as 64 hex characters (defaults to the production network)
    #[arg(long, value_parser = mailbox_server::parse_network_id)]
    network_id: Option<NetworkId>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                "mailbox_server=debug,dashchat_utils=debug,iroh=info,iroh_blobs=info".into()
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let args = Args::parse();

    let signal = tokio::signal::ctrl_c().map(|f| f.expect("failed to listen for event"));
    let listener = tokio::net::TcpListener::bind(&args.addr).await?;
    spawn_server(
        args.db_path.into(),
        listener,
        args.push_notifications_url,
        None,
        Some(RELAY_URL.clone()),
        args.network_id.unwrap_or(*NETWORK_ID),
        signal,
    )
    .await?;

    Ok(())
}
