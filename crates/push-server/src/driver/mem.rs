use std::collections::HashMap;
use tokio::sync::Mutex;

use anyhow::Result;

use crate::{
    driver::Driver,
    types::{FcmToken, PublicKey},
};

pub struct MemDb {
    tokens: Mutex<HashMap<PublicKey, FcmToken>>,
}

impl MemDb {
    pub fn new() -> Self {
        Self {
            tokens: Mutex::new(HashMap::new()),
        }
    }
}

#[async_trait::async_trait]
impl Driver for MemDb {
    async fn store_fcm_token(&self, public_key: &PublicKey, fcm_token: &FcmToken) -> Result<()> {
        self.tokens
            .lock()
            .await
            .insert(public_key.clone(), fcm_token.clone());
        Ok(())
    }

    async fn get_fcm_token(&self, public_key: &PublicKey) -> Result<Option<FcmToken>> {
        Ok(self.tokens.lock().await.get(public_key).cloned())
    }
}
