//! mDNS announcement and discovery of mailbox services on the LAN.
//!
//! [`announce`] registers a mailbox as a DNS-SD service (the instance name is
//! the mailbox's MailboxId) and re-announces it on interface changes.
//! [`discover`] browses the service type, TCP-probes the resolved addresses,
//! and registers each reachable peer straight into a `Mailboxes` manager.
//! Shared by the Tauri app's in-process mailbox and the headless
//! `local-mailbox-server` so the mDNS logic lives in one place.

pub mod announce;
pub mod discover;

pub use announce::{reannounce_on_interface_change_loop, register_mdns_with_retry};
pub use discover::{discover_mailboxes_loop, instance_name_from_fullname};

/// The DNS-SD service type Dash Chat mailboxes announce and browse. The mDNS
/// instance name under this type is the mailbox's MailboxId.
pub const MAILBOX_SERVICE_TYPE: &str = "_dashchat._tcp.local.";
