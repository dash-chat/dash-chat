use derive_more::{Deref, From};
use named_id::RenameAll;
use p2panda_core::PublicKey;
use p2panda_spaces::ActorId;
use serde::{Deserialize, Serialize};
use sqlx::{Sqlite, encode::IsNull, error::BoxDynError, sqlite::SqliteArgumentValue};

/// The ID tied to a particular device.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    From,
    Deref,
    RenameAll,
)]
pub struct DeviceId(PublicKey);

/// The ID for an "agent" which may control multiple devices.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    From,
    Deref,
    RenameAll,
)]
pub struct AgentId(ActorId);

impl AgentId {
    pub fn from_bytes(bytes: &[u8; 32]) -> anyhow::Result<Self> {
        Ok(Self(ActorId::from_bytes(bytes)?))
    }
}

// -- SQLite encoding for DeviceId --

impl sqlx::Type<Sqlite> for DeviceId {
    fn type_info() -> <Sqlite as sqlx::Database>::TypeInfo {
        <Vec<u8> as sqlx::Type<Sqlite>>::type_info()
    }
}

impl sqlx::Encode<'_, Sqlite> for DeviceId {
    fn encode_by_ref(&self, buf: &mut Vec<SqliteArgumentValue<'_>>) -> Result<IsNull, BoxDynError> {
        <Vec<u8> as sqlx::Encode<Sqlite>>::encode(self.as_bytes().to_vec(), buf)
    }
}

impl sqlx::Decode<'_, Sqlite> for DeviceId {
    fn decode(value: <Sqlite as sqlx::Database>::ValueRef<'_>) -> Result<Self, BoxDynError> {
        let bytes = <Vec<u8> as sqlx::Decode<Sqlite>>::decode(value)?;
        let arr: [u8; 32] = bytes.try_into().map_err(|_| "DeviceId is not 32 bytes")?;
        Ok(DeviceId::from(PublicKey::from_bytes(&arr)?))
    }
}

// -- SQLite encoding for AgentId --

impl sqlx::Type<Sqlite> for AgentId {
    fn type_info() -> <Sqlite as sqlx::Database>::TypeInfo {
        <Vec<u8> as sqlx::Type<Sqlite>>::type_info()
    }
}

impl sqlx::Encode<'_, Sqlite> for AgentId {
    fn encode_by_ref(&self, buf: &mut Vec<SqliteArgumentValue<'_>>) -> Result<IsNull, BoxDynError> {
        <Vec<u8> as sqlx::Encode<Sqlite>>::encode(self.as_bytes().to_vec(), buf)
    }
}

impl sqlx::Decode<'_, Sqlite> for AgentId {
    fn decode(value: <Sqlite as sqlx::Database>::ValueRef<'_>) -> Result<Self, BoxDynError> {
        let bytes = <Vec<u8> as sqlx::Decode<Sqlite>>::decode(value)?;
        let arr: [u8; 32] = bytes.try_into().map_err(|_| "AgentId is not 32 bytes")?;
        Ok(AgentId(ActorId::from_bytes(&arr)?))
    }
}
