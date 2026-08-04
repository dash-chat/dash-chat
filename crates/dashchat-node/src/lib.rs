pub mod blob_sync;
mod chat;
mod contact;
mod error;
mod filesystem;
mod network_change_notifier;
pub mod node;
mod payload;
pub mod stores;
pub mod topic;
mod unfetched_blobs;
pub mod util;

mod id;
pub mod mailbox;

pub mod compat;
#[cfg(feature = "testing")]
pub mod testing;

pub use aliased::Aliasing;

pub use chat::*;
pub use contact::{QrCode, ShareIntent};
pub use error::{
    AddContactError, DeleteMessageError, EditMessageError, Error, RemoveGroupMemberError,
};
pub use id::*;
pub use node::{Node, NodeConfig, Notification, OpNotification, SystemNotification};
pub use p2panda::SigningKey;
pub use p2panda_spaces::ActorId;
pub use payload::*;
pub use topic::{Topic, TopicId};
pub use unfetched_blobs::{
    LocalStoreBlobTracker, followup_unfetched_blobs_once, spawn_unfetched_blob_followup_task,
};

pub trait Cbor: serde::Serialize + serde::de::DeserializeOwned {
    fn as_bytes(&self) -> Result<Vec<u8>, p2panda_core::cbor::EncodeError> {
        p2panda_core::cbor::encode_cbor(&self)
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self, p2panda_core::cbor::DecodeError> {
        p2panda_core::cbor::decode_cbor(bytes)
    }
}

pub trait AsBody: Cbor {
    fn try_into_body(&self) -> Result<p2panda_core::Body, p2panda_core::cbor::EncodeError> {
        let bytes = self.as_bytes()?;
        Ok(p2panda_core::Body::new(bytes.as_slice()))
    }

    fn try_from_body(body: &p2panda_core::Body) -> Result<Self, p2panda_core::cbor::DecodeError> {
        Self::from_bytes(body.to_bytes().as_slice())
    }

    fn try_from_body_opt(
        body: Option<&p2panda_core::Body>,
    ) -> Result<Option<Self>, p2panda_core::cbor::DecodeError> {
        body.map(|body| Self::try_from_body(body)).transpose()
    }
}
