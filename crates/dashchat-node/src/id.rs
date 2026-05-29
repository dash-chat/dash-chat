use derive_more::{Deref, From, derive::Display};
use p2panda::VerifyingKey;
use p2panda_spaces::ActorId;
use serde::{Deserialize, Serialize};
use sqlx::{Sqlite, encode::IsNull, error::BoxDynError, sqlite::SqliteArgumentValue};

/// The ID tied to a particular device.
#[derive(
    Clone,
    Copy,
    Debug,
    Display,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    From,
    Deref,
)]
pub struct DeviceId(VerifyingKey);

impl named_id::Nameable for DeviceId {
    fn shortener(&self) -> Option<named_id::Shortener> {
        Some(named_id::Shortener {
            length: 4,
            prefix: "D",
        })
    }
}

/// The ID for an "agent" which may control multiple devices.
#[derive(
    Clone,
    Copy,
    Debug,
    Display,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    From,
    Deref,
)]
pub struct AgentId(ActorId);

impl named_id::Nameable for AgentId {
    fn shortener(&self) -> Option<named_id::Shortener> {
        Some(named_id::Shortener {
            length: 4,
            prefix: "A",
        })
    }
}

impl AgentId {
    pub fn from_bytes(bytes: &[u8; 32]) -> anyhow::Result<Self> {
        Ok(Self(ActorId::from_bytes(bytes)?))
    }
}

// TODO: when device groups are implemented, this switches to AgentId.
pub type ChatMember = DeviceId;

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
        Ok(DeviceId::from(VerifyingKey::from_bytes(&arr)?))
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
