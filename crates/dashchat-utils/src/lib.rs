pub mod blob_sync;
mod derive_watch;
pub mod endpoint;
mod fetch_loop;
mod retry_with_backoff;
mod singleton_task_with_retries;

pub use derive_watch::derive_watch;
pub use fetch_loop::{fetch_loop, FetchConfig, FetchPool};
pub use retry_with_backoff::retry_with_backoff;
pub use singleton_task_with_retries::SingletonTaskWithRetries;

pub const NETWORK_ID: &[u8; 32] = b"usability, reliability, security";

pub static RELAY_URL: std::sync::LazyLock<iroh::RelayUrl> = std::sync::LazyLock::new(|| {
    "https://euc1-1.relay.guillemcordoba.dash-chat.iroh.link/"
        .parse()
        .expect("valid relay URL")
});
