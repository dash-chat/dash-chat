#[cfg(feature = "cassandra")]
pub mod cassandra;

use crate::types::PublicKey;

#[async_trait::async_trait]
pub trait Driver: Send + Sync + 'static {
    async fn store_fcm_token(&self, public_key: &PublicKey, fcm_token: &str) -> anyhow::Result<()>;
    async fn get_fcm_token(&self, public_key: &PublicKey) -> anyhow::Result<Option<String>>;
}
