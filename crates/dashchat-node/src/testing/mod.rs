mod behavior;
mod introduce;
mod mailbox;
mod test_node;

pub use introduce::*;
pub use mailbox::*;
pub use test_node::*;
use tracing_subscriber::EnvFilter;

/// Appends a random suffix to `bytes` so blob content (and therefore its
/// content-addressed hash) is unique per call. Tests running against a
/// persistent/shared mailbox must not reuse blob content across runs, or
/// their blob hashes collide with earlier runs' blobs.
pub fn unique_blob_bytes(bytes: impl Into<Vec<u8>>) -> Vec<u8> {
    let mut bytes = bytes.into();
    bytes.extend_from_slice(&rand::random::<[u8; 16]>());
    bytes
}

pub fn setup_tracing(dirs: &[&str], more: bool) {
    // Ensure aliases are set up. Idempotent.
    crate::util::setup_aliases();

    let dirs = dirs.join(",");
    let filter = EnvFilter::try_new(dirs).unwrap();
    let _ = tracing_subscriber::fmt::fmt()
        .with_thread_names(false)
        .with_target(more)
        .with_file(more)
        .with_line_number(more)
        .with_env_filter(filter)
        .try_init();
}
