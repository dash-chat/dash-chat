mod behavior;
mod introduce;
mod mailbox;
mod test_node;

pub use introduce::*;
pub use mailbox::*;
pub use test_node::*;
use tracing_subscriber::EnvFilter;

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
