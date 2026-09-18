#[cfg(feature = "iroh")]
pub mod blob_sync;
mod derive_watch;
#[cfg(feature = "iroh")]
pub mod endpoint;
mod fetch_loop;
mod retry_with_backoff;
mod singleton_task_with_retries;

#[cfg(feature = "cbor")]
pub mod cbor;

pub use derive_watch::derive_watch;
pub use fetch_loop::{fetch_loop, FetchConfig, FetchPool};
pub use retry_with_backoff::retry_with_backoff;
pub use singleton_task_with_retries::SingletonTaskWithRetries;

pub const NETWORK_ID: &[u8; 32] = b"usability, reliability, security";

/// The p2p network e2e agents live on. An e2e build is given an
/// `E2E_NETWORK_ID`, hashed to the id's width, so test runs sharing a LAN,
/// each built with its own id, refuse each other's peers; a build without one
/// gets the id every such build shares.
#[cfg(feature = "iroh")]
pub fn e2e_network_id() -> [u8; 32] {
    match option_env!("E2E_NETWORK_ID") {
        Some(id) => *iroh_blobs::Hash::new(format!("dashchat e2e {id}")).as_bytes(),
        None => *b"dashchat end-to-end test network",
    }
}

/// The network id a mailbox's blob ALPN is hashed with, which has to be the
/// one the apps dialing it use: the e2e network for a build given an
/// `E2E_NETWORK_ID`, the production network otherwise.
#[cfg(feature = "iroh")]
pub fn mailbox_network_id() -> [u8; 32] {
    if option_env!("E2E_NETWORK_ID").is_some() {
        e2e_network_id()
    } else {
        *NETWORK_ID
    }
}

#[cfg(feature = "iroh")]
pub static RELAY_URL: std::sync::LazyLock<iroh::RelayUrl> = std::sync::LazyLock::new(|| {
    option_env!("E2E_RELAY_URL")
        .unwrap_or("https://euc1-1.relay.guillemcordoba.dash-chat.iroh.link/")
        .parse()
        .expect("valid relay URL")
});
