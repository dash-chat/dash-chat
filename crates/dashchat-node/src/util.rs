#![allow(unused)]

use p2panda::VerifyingKey;
use p2panda_spaces::ActorId;

pub trait ResultExt<T, E> {
    fn ok_or_warn(self, message: &str) -> Option<T>;
}

impl<T, E> ResultExt<T, E> for Result<T, E>
where
    E: std::fmt::Debug,
{
    fn ok_or_warn(self, message: &str) -> Option<T> {
        self.map_err(|e| {
            tracing::warn!("{}: {:?}", message, e);
            e
        })
        .ok()
    }
}

pub fn first<T, U>(pair: (T, U)) -> T {
    pair.0
}

pub fn second<T, U>(pair: (T, U)) -> U {
    pair.1
}

#[derive(Clone)]
pub(crate) struct CancelAndWait<R> {
    handle: std::sync::Arc<tokio::sync::Mutex<Option<tokio::task::JoinHandle<R>>>>,
    token: tokio_util::sync::CancellationToken,
}

impl<R> CancelAndWait<R> {
    pub fn new(
        handle: tokio::task::JoinHandle<R>,
        token: tokio_util::sync::CancellationToken,
    ) -> Self {
        Self {
            handle: std::sync::Arc::new(tokio::sync::Mutex::new(Some(handle))),
            token,
        }
    }

    pub async fn cancel_and_wait(&self) -> Option<Result<R, tokio::task::JoinError>> {
        self.token.cancel();
        Some(self.handle.lock().await.take()?.await)
    }
}

/// Clamp a hash to a valid ed25519 public key.
pub fn clamp_to_ed25519_pubkey(mut hash: [u8; 32]) -> VerifyingKey {
    hash[0] &= 248;
    hash[31] &= 127;
    hash[31] |= 64;
    let signing_key = ed25519_dalek::SigningKey::from_bytes(&hash);
    let pubkey = signing_key.verifying_key();
    VerifyingKey::from_bytes(pubkey.as_bytes()).unwrap()
}

pub fn setup_aliases() {
    use aliased::Aliasing;
    crate::AgentId::alias_prefix("A");
    crate::DeviceId::alias_prefix("D");
    p2panda::VerifyingKey::alias_prefix("K");
    p2panda::Hash::alias_prefix("H");
    crate::topic::TopicId::alias_prefix("T");
    p2panda::Topic::alias_prefix("t");
}
