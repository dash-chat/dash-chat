use derive_more::{
    Deref, From,
    derive::{Display, Into},
};
use p2panda::VerifyingKey;
use p2panda_spaces::ActorId;
use serde::{Deserialize, Serialize};
use sqlx::{Sqlite, encode::IsNull, error::BoxDynError};

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

impl std::str::FromStr for DeviceId {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(DeviceId::from(VerifyingKey::from_str(s)?))
    }
}

impl DeviceId {
    /// Hex-encoded bytes of the underlying verifying key.
    pub fn to_hex(&self) -> String {
        self.0.to_hex()
    }
}

impl mailbox_client::MailboxKey for DeviceId {
    fn to_mailbox_key(&self) -> String {
        self.to_hex()
    }

    fn from_mailbox_key(key: &str) -> Result<Self, anyhow::Error> {
        Ok(key.parse()?)
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

impl AgentId {
    pub fn from_bytes(bytes: &[u8; 32]) -> anyhow::Result<Self> {
        Ok(Self(ActorId::from_bytes(bytes)?))
    }
}

#[deprecated = "XXX: represents our current false equivalence between DeviceId and AgentId. This must be removed and cleaned up before device groups can work."]
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
    Into,
    Deref,
)]
pub struct FakeAgentId(DeviceId);

// TODO: when device groups are implemented, this switches to AgentId.
pub type ChatMember = DeviceId;

// -- SQLite encoding for DeviceId --

impl sqlx::Type<Sqlite> for DeviceId {
    fn type_info() -> <Sqlite as sqlx::Database>::TypeInfo {
        <Vec<u8> as sqlx::Type<Sqlite>>::type_info()
    }
}

impl sqlx::Encode<'_, Sqlite> for DeviceId {
    fn encode_by_ref(
        &self,
        buf: &mut <Sqlite as sqlx::Database>::ArgumentBuffer,
    ) -> Result<IsNull, BoxDynError> {
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
    fn encode_by_ref(
        &self,
        buf: &mut <Sqlite as sqlx::Database>::ArgumentBuffer,
    ) -> Result<IsNull, BoxDynError> {
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

#[cfg(test)]
mod tests {
    use mailbox_client::MailboxKey;
    use p2panda::SigningKey;

    use super::*;

    // Hex-encoded verifying key for SigningKey::from_bytes(&[0xcd; 32]).
    // Ed25519 clamping means the public key is NOT 0xcd...cd.
    const DEVICE_HEX: &str = "fc947730f49eb01427a66e050733294d9e520e545c7a27125a780634e0860a27";

    #[test]
    fn device_id_round_trips_through_mailbox_key() {
        let device_id = DeviceId::from(SigningKey::from_bytes(&[0xcd; 32]).verifying_key());
        let key = device_id.to_mailbox_key();
        assert_eq!(key, DEVICE_HEX);
        assert_eq!(DeviceId::from_mailbox_key(&key).unwrap(), device_id);
    }

    #[test]
    fn device_id_from_mailbox_key_rejects_non_hex() {
        assert!(DeviceId::from_mailbox_key("not-hex").is_err());
    }

    #[test]
    fn device_id_from_mailbox_key_rejects_wrong_length() {
        assert!(DeviceId::from_mailbox_key("cd").is_err());
        assert!(
            DeviceId::from_mailbox_key(
                "cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd"
            )
            .is_err()
        );
    }

    #[test]
    fn device_id_from_mailbox_key_rejects_invalid_curve_point() {
        // 32 valid hex bytes that do not form a valid Ed25519 public key.
        assert!(
            DeviceId::from_mailbox_key(
                "0000000000000000000000000000000000000000000000000000000000000001"
            )
            .is_err()
        );
    }
}
