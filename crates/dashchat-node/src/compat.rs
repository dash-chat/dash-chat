use p2panda_core::cbor::{decode_cbor, encode_cbor};
use serde::{Deserialize, Serialize};
use sqlx::{Sqlite, encode::IsNull, error::BoxDynError, sqlite::SqliteArgumentValue};

dashchat_compat::capabilities! {
    #[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Capabilities {
        messaging: 1,
    }
}

impl sqlx::Type<Sqlite> for Capabilities {
    fn type_info() -> <Sqlite as sqlx::Database>::TypeInfo {
        <Vec<u8> as sqlx::Type<Sqlite>>::type_info()
    }
}

impl sqlx::Encode<'_, Sqlite> for Capabilities {
    fn encode_by_ref(&self, buf: &mut Vec<SqliteArgumentValue<'_>>) -> Result<IsNull, BoxDynError> {
        let bytes = encode_cbor(self)?;
        <Vec<u8> as sqlx::Encode<Sqlite>>::encode(bytes, buf)
    }
}

impl sqlx::Decode<'_, Sqlite> for Capabilities {
    fn decode(value: <Sqlite as sqlx::Database>::ValueRef<'_>) -> Result<Self, BoxDynError> {
        let bytes = <Vec<u8> as sqlx::Decode<Sqlite>>::decode(value)?;
        Ok(decode_cbor(bytes.as_slice())?)
    }
}
