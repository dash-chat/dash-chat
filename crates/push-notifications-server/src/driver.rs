#[cfg(feature = "cassandra")]
pub mod cassandra;
pub mod mem;

use crate::types::{FcmToken, PublicKey};

#[async_trait::async_trait]
pub trait Driver: Send + Sync + 'static {
    async fn store_fcm_token(
        &self,
        public_key: &PublicKey,
        fcm_token: &FcmToken,
    ) -> anyhow::Result<()>;

    async fn get_fcm_token(&self, public_key: &PublicKey) -> anyhow::Result<Option<FcmToken>>;
}
