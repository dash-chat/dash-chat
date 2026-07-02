use serde::Serialize;
use thiserror::Error;
use tokio::sync::oneshot::error::RecvError;

#[derive(Debug, Error, Serialize)]
#[serde(tag = "kind", content = "message")]
pub enum Error {
    #[error("Failed to initialize topic: {0}")]
    InitializeTopic(String),

    #[error("Failed to register bootstrap node: {0}")]
    RegisterBootstrap(String),

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

    #[error("Failed to store contact info: {0}")]
    StoreContact(String),

    #[error(transparent)]
    #[serde(untagged)]
    Common(#[from] Error),
}

#[derive(Debug, Error, Serialize)]
#[serde(tag = "kind", content = "message")]
pub enum RemoveGroupMemberError {
    #[error("cannot remove the only admin from a group that still has members")]
    LastAdmin,

    #[error("Failed to remove group member: {0}")]
    InternalServerError(String),
}

impl From<anyhow::Error> for RemoveGroupMemberError {
    fn from(e: anyhow::Error) -> Self {
        RemoveGroupMemberError::InternalServerError(e.to_string())
    }
}

#[derive(Debug, Error)]
pub enum ShutdownError {
    #[error("Error sending shutting signal to node actor: {0:?}")]
    ActorShutdown(Box<dyn std::any::Any + Send + 'static>),

    #[error("Error closing down the processor task: {0:?}")]
    ProcessorCancel(Box<dyn std::any::Any + Send + 'static>),

    #[error(transparent)]
    Receive(#[from] RecvError),
}
