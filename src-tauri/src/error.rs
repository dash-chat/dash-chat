use serde::Serialize;
use thiserror::Error as ThisError;

/// Host-side error for node-backed Tauri commands.
///
/// `NodeNotReady` is a host concern, not a core-node one (the managed node can be
/// temporarily absent: the startup race before `async_setup` manages it, or the
/// iOS app quiescing/rebuilding it across background/foreground), so it lives here
/// rather than in `dashchat-node`. The underlying node errors are wrapped untagged
/// so their `{kind, message}` serialization reaches the frontend unchanged, and
/// `NodeNotReady`'s message carries the retry sentinel `invokeAfterSetup` matches.
#[derive(Debug, ThisError, Serialize)]
#[serde(tag = "kind", content = "message")]
pub enum Error {
    #[error("{0}")]
    NodeNotReady(String),

    #[error(transparent)]
    #[serde(untagged)]
    Node(#[from] dashchat_node::Error),

    #[error(transparent)]
    #[serde(untagged)]
    AddContact(#[from] dashchat_node::AddContactError),
}
