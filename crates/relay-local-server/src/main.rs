use std::net::SocketAddr;

use clap::Parser;
use iroh_relay::server::{RelayConfig, Server, ServerConfig};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Parser, Debug)]
#[command(name = "relay-local-server")]
#[command(about = "A plain-HTTP iroh relay for the e2e harness, so a run controls whether peers can reach each other through a relay", long_about = None)]
struct Args {
    /// Address to serve on
    #[arg(long, default_value = "0.0.0.0:3340")]
    addr: SocketAddr,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    panic_policy::install_panic_hook();

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "relay_local_server=info,iroh_relay=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let args = Args::parse();
    // `ServerConfig` is non-exhaustive, so it can't be built with a literal.
    let mut config = ServerConfig::default();
    config.relay = Some(RelayConfig::new(args.addr));
    let mut server = Server::spawn(config).await?;
    tracing::info!("relay serving on http://{}", args.addr);

    tokio::select! {
        _ = tokio::signal::ctrl_c() => server.shutdown().await?,
        // Already stopped: there is nothing left to shut down.
        _ = server.join() => {}
    }
    Ok(())
}
