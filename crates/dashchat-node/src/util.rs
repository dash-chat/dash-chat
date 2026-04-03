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

/// Clamp the hash to a valid ed25519 public key.
/// This is used to create valid chat topics, which p2panda requires to be valid pubkeys.
pub fn ed25519_pubkey_from_hash(mut hash: [u8; 32]) -> PublicKey {
    hash[0] &= 248;
    hash[31] &= 127;
    hash[31] |= 64;
    let signing_key = ed25519_dalek::SigningKey::from_bytes(&hash);
    let pubkey = signing_key.verifying_key();
    PublicKey::from_bytes(pubkey.as_bytes()).unwrap()
}

pub fn first<T, U>(pair: (T, U)) -> T {
    pair.0
}

pub fn second<T, U>(pair: (T, U)) -> U {
    pair.1
}
