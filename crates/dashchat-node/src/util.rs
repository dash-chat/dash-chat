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

pub fn max_width<R: named_id::Rename>(items: impl IntoIterator<Item = R>) -> usize {
    items
        .into_iter()
        .map(|item| item.renamed().to_string().len())
        .max()
        .unwrap_or(0)
}

#[deprecated = "need a more certain way to know that an ActorId is actually a pubkey"]
pub fn actor_to_pubkey(actor: ActorId) -> PublicKey {
    PublicKey::from_bytes(actor.as_bytes()).unwrap()
}
