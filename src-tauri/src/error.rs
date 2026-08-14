use serde::Serialize;
use thiserror::Error as ThisError;

/// Host-side error for node-backed Tauri commands.
#[derive(Debug, ThisError, Serialize)]
#[serde(tag = "kind", content = "message")]
pub enum Error {
    #[error("NodeNotReady")]
    NodeNotReady,

    #[error(transparent)]
    #[serde(untagged)]
    Node(#[from] dashchat_node::Error),

    #[error(transparent)]
    #[serde(untagged)]
    AddContact(#[from] dashchat_node::AddContactError),
}

/// Lets commands that still return `Result<_, String>` use `?` on `AppNodeManager::get`
/// and other `Error`-producing calls.
impl From<Error> for String {
    fn from(err: Error) -> Self {
        err.to_string()
    }
}
