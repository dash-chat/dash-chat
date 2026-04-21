#![allow(unused)]

use p2panda_core::PublicKey;
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

#[deprecated = "need a more certain way to know that an ActorId is actually a pubkey"]
pub fn actor_to_pubkey(actor: ActorId) -> PublicKey {
    PublicKey::from_bytes(actor.as_bytes()).unwrap()
}

pub fn first<T, U>(pair: (T, U)) -> T {
    pair.0
}

pub fn second<T, U>(pair: (T, U)) -> U {
    pair.1
}
