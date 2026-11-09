//! Media blobs are encrypted towards the chat's space before entering the blob store, so a
//! blob's hash is the hash of its ciphertext and identical media has different hashes in
//! different contexts. This module holds what the node needs to decrypt them again.
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use p2panda_encryption::crypto::xchacha20::XAeadNonce;
use p2panda_encryption::data_scheme::GroupSecretId;

use crate::chat::MediaMetadata;
use crate::topic::TopicId;

/// Domain label for the HKDF subkey media blobs are encrypted with.
pub const BLOB_AUX_INFO: &[u8] = b"dashchat/blob/v1";

/// What is needed to decrypt a media blob: the chat topic whose space holds the secret, plus
/// the pinned group secret id and nonce from the message's [`MediaMetadata`].
#[derive(Clone, Copy, Debug)]
pub struct BlobDecryptInfo {
    pub topic: TopicId,
    pub group_secret_id: GroupSecretId,
    pub nonce: XAeadNonce,
}

/// In-memory map from a media blob's (ciphertext) hash to its decryption parameters.
///
/// Rebuilt from stored operations at startup (alongside the blob fetch pool) and kept up to
/// date as messages are sent, processed and deleted.
#[derive(Clone, Default)]
pub struct BlobDecryptIndex {
    inner: Arc<Mutex<HashMap<iroh_blobs::Hash, BlobDecryptInfo>>>,
}

impl BlobDecryptIndex {
    pub fn insert_blobs(&self, topic: TopicId, media: &[MediaMetadata]) {
        let mut map = self.inner.lock().unwrap();
        for item in media {
            map.insert(
                item.hash(),
                BlobDecryptInfo {
                    topic,
                    group_secret_id: item.group_secret_id(),
                    nonce: item.nonce(),
                },
            );
        }
    }

    pub fn get(&self, hash: &iroh_blobs::Hash) -> Option<BlobDecryptInfo> {
        self.inner.lock().unwrap().get(hash).copied()
    }

    pub fn remove(&self, hashes: &[iroh_blobs::Hash]) {
        let mut map = self.inner.lock().unwrap();
        for hash in hashes {
            map.remove(hash);
        }
    }
}
