use serde::Serialize;
use thiserror::Error as ThisError;

/// Host-side error for node-backed Tauri commands.
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
