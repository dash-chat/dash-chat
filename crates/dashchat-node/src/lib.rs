#![feature(bool_to_result)]

mod chat;
mod contact;
mod error;
mod filesystem;
pub mod node;
mod payload;
pub mod stores;
pub mod topic;
mod util;

pub mod polestar;

mod id;
pub mod local_store;
pub mod mailbox;

#[cfg(feature = "testing")]
pub mod testing;

use named_id::*;

pub use chat::*;
pub use contact::{ContactCode, InboxTopic, ShareIntent};
pub use error::{AddContactError, ContactCodeError, Error};
pub use id::*;
pub use node::{LocalStore, Node, NodeConfig, Notification};
pub use p2panda_core::PrivateKey;
pub use p2panda_spaces::ActorId;
pub use payload::*;
pub use topic::Topic;

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
}

pub fn timestamp_now() -> u64 {
    use std::time::SystemTime;
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("time from operation system")
        .as_secs()
}
