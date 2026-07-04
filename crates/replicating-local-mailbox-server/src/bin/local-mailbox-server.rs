//! Dash Chat LAN mailbox appliance binary.
//!
//! A full Dash Chat `mailbox-server` wrapped with:
//!   1. **mDNS announce** — publishes `_dashchat._tcp.local.` with the mailbox's
//!      `MailboxId` as the instance name, so Dash Chat clients auto-discover it.
//!   2. **mDNS discovery** — browses for other mailboxes and syncs with them.
//!   3. **Replication** — bidirectional blip + blob sync against each peer, plus
//!      an optional cloud bridge.

use std::time::Duration;

use anyhow::Context;
use clap::Parser;
use mdns_sd::ServiceDaemon;
use replicating_local_mailbox_server::ReplicatingLocalMailboxServer;

#[derive(Parser, Debug)]
#[command(
    name = "local-mailbox-server",
    about = "Dash Chat LAN mailbox: server + mDNS announce/discovery + replication"
)]
struct Args {
    /// Path to the redb database file. Holds the persistent server identity
    /// (and therefore the stable MailboxId) plus all stored blips.
    #[arg(long, default_value = "/var/lib/dashchat-mailbox/mailbox.redb")]
    db_path: std::path::PathBuf,

    /// Address to bind the HTTP server to. Dual-stack `[::]` accepts IPv4 and
    /// IPv6 so peers reach us over whichever address the mDNS record announces.
    #[arg(long, default_value = "[::]:3000")]
    addr: String,

    /// mDNS service type to announce and browse. Defaults to the canonical Dash
    /// Chat mailbox service type.
    #[arg(long, default_value = mailbox_mdns_discovery::MAILBOX_SERVICE_TYPE)]
    service_type: String,

    /// Replicate with the cloud/remote mailbox at this URL, bridging the LAN to
    /// the internet. Unset means no cloud bridge.
    #[arg(long)]
    cloud_url: Option<String>,

    /// Seconds between replication sync passes.
    #[arg(long, default_value_t = 30)]
    sync_interval: u64,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    init_tracing();

    let daemon = ServiceDaemon::new().context("failed to start mDNS daemon")?;
    let server = ReplicatingLocalMailboxServer::spawn(
        args.db_path,
        &args.addr,
        daemon,
        args.service_type,
        Duration::from_secs(args.sync_interval),
        args.cloud_url,
    )
    .await
    .context("failed to start local mailbox server")?;
    tracing::info!("MailboxId (mDNS instance name): {}", server.mailbox_id());
    tracing::info!("Mailbox HTTP server listening on port {}", server.port());

    tokio::signal::ctrl_c().await?;
    tracing::info!("Shutting down");
    server.stop().await;
    Ok(())
}

fn init_tracing() {
    // `fmt().init()` also installs the log->tracing bridge (tracing-subscriber's
    // default `tracing-log` feature), so `log` records from mailbox-mdns-discovery
    // and the announce helpers show up alongside the server's own output. Don't
    // call `tracing_log::LogTracer::init()` here as well — a second global logger
    // install makes `init()` panic with SetLoggerError.
    let filter = std::env::var("RUST_LOG").unwrap_or_else(|_| {
        "local_mailbox_server=info,replicating_local_mailbox_server=info,mailbox_local_server=info,mailbox_server=info,mailbox_client=info,mailbox_mdns_discovery=info".to_string()
    });
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::new(filter))
        .init();
}
