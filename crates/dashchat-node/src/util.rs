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

pub fn setup_aliases() {
    use aliased::Aliasing;
    crate::AgentId::alias_prefix("A");
    crate::DeviceId::alias_prefix("D");
    p2panda::VerifyingKey::alias_prefix("K");
    p2panda::Hash::alias_prefix("H");
    crate::topic::TopicId::alias_prefix("T");
    p2panda::Topic::alias_prefix("t");
}
