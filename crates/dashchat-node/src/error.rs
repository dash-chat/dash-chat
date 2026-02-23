use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error, Serialize)]
#[serde(tag = "kind", content = "message")]
pub enum Error {
    #[error("Failed to initialize topic: {0}")]
    InitializeTopic(String),

    #[error("Failed to author operation: {0}")]
    AuthorOperation(String),

    #[error("Failed to add active inbox: {0}")]
    AddActiveInbox(String),

    #[error("Failed to get active inboxes: {0}")]
    GetActiveInboxes(String),
}

#[derive(Debug, Error, Serialize)]
#[serde(tag = "kind", content = "message")]
pub enum AddContactError {
    #[error("Profile must be created before adding contacts")]
    ProfileNotCreated,

    #[error("Failed to create QR code: {0}")]
    CreateQrCode(String),

    #[error("Failed to create direct chat: {0}")]
    CreateDirectChat(String),

    #[error(transparent)]
    #[serde(untagged)]
    Common(#[from] Error),
}
