//! Cross-mailbox orchestration: registration, the poll loop, sync scheduling,
//! watermark persistence, and blob-upload tracking across every mailbox a node
//! uses. Builds on the per-mailbox protocol and connection primitives in
//! [`mailbox_client`]; the dependency is one-way (this crate depends on
//! `mailbox-client`, never the reverse).

pub mod blobs;
mod mailbox_registry;
pub mod manager;
pub(crate) mod persistence;
mod polling;
mod sync_tracker;

pub use blobs::unfetched_blobs::BlobSource;
pub use sync_tracker::{MailboxSyncState, MailboxSyncTracker};

// The per-mailbox vocabulary, re-exported so downstream crates need only
// depend on `mailboxes-manager`.
#[cfg(feature = "testing")]
pub use mailbox_client::testing;
pub use mailbox_client::{
    BlobReader, BlobUploadLifecycle, FetchRequest, FetchResponse, FetchTopicResponse, HTTP_CLIENT,
    ItemTraits, LastError, MailboxClient, MailboxConnectionState, MailboxId, MailboxItem,
    MailboxKey, MailboxesConfig, NoopUnfetchedBlobTracker, OptionalItemTraits, PublishResponse,
    RegisterPeerRequest, SeqNum, SyncStatus, UnfetchedBlobTracker, backends, store,
};
