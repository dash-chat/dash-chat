//! Cross-mailbox orchestration: registration, the poll loop, sync scheduling,
//! watermark persistence, and blob-upload tracking across every mailbox a node
//! uses. Builds on the per-mailbox protocol and connection primitives in
//! `mailbox-client`; the dependency is one-way (this crate depends on
//! `mailbox-client`, never the reverse), and downstream crates import from
//! whichever of the two owns the concept they use.

pub mod blobs;
mod mailbox_registry;
pub mod manager;
pub(crate) mod persistence;
mod polling;
mod sync_tracker;
mod tracked_mailbox;

pub use blobs::unfetched_blobs::BlobSource;
pub use manager::Mailboxes;
pub use sync_tracker::{MailboxSyncState, MailboxSyncTracker};
pub use tracked_mailbox::TrackedMailbox;
