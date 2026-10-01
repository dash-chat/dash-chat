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

/// A position in an append-only log, shared by every layer that handles one:
/// p2panda operations, the op store, and the mailbox protocol.
///
/// MUST stay the same type as `p2panda_core::SeqNum`; `dashchat-node` asserts
/// that at compile time. The two used to differ in width, which silently
/// narrowed sequence numbers wherever the store and the mailbox met.
pub type SeqNum = u32;

pub const NETWORK_ID: &[u8; 32] = b"usability, reliability, security";

#[cfg(feature = "iroh")]
pub static RELAY_URL: std::sync::LazyLock<iroh::RelayUrl> = std::sync::LazyLock::new(|| {
    option_env!("E2E_RELAY_URL")
        .unwrap_or("https://euc1-1.relay.guillemcordoba.dash-chat.iroh.link/")
        .parse()
        .expect("valid relay URL")
});
