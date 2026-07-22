use std::collections::{BTreeSet, HashMap};

use p2panda::Hash;
use serde::Serialize;
use thiserror::Error;

use crate::DeviceId;

use super::edit::EDIT_WINDOW_MICROS;
use super::{ChatOp, ChatOpKind, ValidChatOps};

/// The window during which a message may be deleted for everyone, measured from
/// the original message timestamp. Matches the edit window (24h in µs).
pub const DELETE_WINDOW_MICROS: u64 = EDIT_WINDOW_MICROS;

/// Why a delete operation is considered invalid (not allowed to be applied).
///
/// The same validation rules are enforced on the author's side (as a hard error before
/// publishing) and on the receiving side (the delete is ignored with a warning).
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize)]
#[serde(tag = "kind", content = "message")]
pub enum DeleteError {
    #[error("the message being deleted could not be found in this chat")]
    TargetNotFound,

    #[error("only messages and their edits can be deleted")]
    TargetNotDeletable,

    #[error("a message can only be deleted by its original author")]
    NotAuthor,

    #[error("a delete must target the most recent edit of a message")]
    NotLatestEdit,

    #[error("a delete must cover the entire edit chain of a message")]
    IncompleteChain,

    #[error("this message is already deleted")]
    AlreadyDeleted,

    #[error("the delete window for this message has expired")]
    WindowExpired,
}

/// Collect the full set of hashes comprising the edit chain
/// ending at `target` by traversing backwards through references
/// until reaching the original message.
///
/// `target` must be the most
/// recent edit of its chain (or the message itself when unedited) — deleting
/// anything but the tip is rejected with [`DeleteError::NotLatestEdit`].
///
/// This is the author-side helper that builds the hash set for a
/// `DeleteMessage` payload. Note that the set is unordered.
pub fn collect_deletable_edit_chain(
    valid_ops: &HashMap<Hash, ChatOp>,
    target: &Hash,
) -> Result<BTreeSet<Hash>, DeleteError> {
    let target_op = valid_ops.get(target).ok_or(DeleteError::TargetNotFound)?;
    match target_op.kind {
        ChatOpKind::Message | ChatOpKind::Edit(_) => {}
        ChatOpKind::Delete(_) | ChatOpKind::Other => return Err(DeleteError::TargetNotDeletable),
    }

    let tip_is_edited = valid_ops
        .values()
        .any(|op| matches!(&op.kind, ChatOpKind::Edit(t) if t == target));
    if tip_is_edited {
        return Err(DeleteError::NotLatestEdit);
    }

    let mut chain = BTreeSet::new();
    let mut current = *target;
    for _ in 0..valid_ops.len() + 1 {
        chain.insert(current);
        let op = valid_ops.get(&current).ok_or(DeleteError::TargetNotFound)?;
        match &op.kind {
            ChatOpKind::Message => return Ok(chain),
            ChatOpKind::Edit(t) => current = *t,
            ChatOpKind::Delete(_) | ChatOpKind::Other => {
                return Err(DeleteError::TargetNotDeletable);
            }
        }
    }
    // Cyclic chain: cannot happen for chains built by valid edits.
    Err(DeleteError::IncompleteChain)
}

/// Walk backwards from `target` through the edit chain to the original
/// `Message` operation and return its hash. Unlike
/// [`collect_deletable_edit_chain`] this imposes no "must be the latest edit"
/// restriction — `target` may be any operation in the chain — because
/// delete-for-me deletes a whole message regardless of which version the caller
/// happened to point at.
pub fn resolve_message_root(
    valid_ops: &HashMap<Hash, ChatOp>,
    target: &Hash,
) -> Result<Hash, DeleteError> {
    let mut current = *target;
    for _ in 0..valid_ops.len() + 1 {
        let op = valid_ops.get(&current).ok_or(DeleteError::TargetNotFound)?;
        match &op.kind {
            ChatOpKind::Message => return Ok(current),
            ChatOpKind::Edit(edit_hash) => current = *edit_hash,
            ChatOpKind::Delete(_) | ChatOpKind::Other => {
                return Err(DeleteError::TargetNotDeletable);
            }
        }
    }
    // Cyclic chain: cannot happen for chains built by valid edits.
    Err(DeleteError::IncompleteChain)
}

/// Every operation reachable forward from `root` through the edit graph: the
/// root plus every edit that (transitively) targets it. Used to tombstone a
/// whole message chain given only its original op. Only ops present in
/// `valid_ops` (i.e. still carrying a body) are reachable — already body-less
/// members carry no `edit_hash` pointer and don't need re-tombstoning.
pub fn forward_edit_closure(
    valid_ops: &HashMap<Hash, ChatOp>,
    root: Hash,
) -> BTreeSet<Hash> {
    let mut chain = BTreeSet::from([root]);
    loop {
        let mut grew = false;
        for (hash, op) in valid_ops {
            if let ChatOpKind::Edit(target) = &op.kind {
                if chain.contains(target) && chain.insert(*hash) {
                    grew = true;
                }
            }
        }
        if !grew {
            break;
        }
    }
    chain
}

pub struct DeleteCandidate {
    pub hashes: BTreeSet<Hash>,
    pub deleter: DeviceId,
    pub delete_timestamp: u64,
    pub self_hash: Option<Hash>,
}

impl DeleteCandidate {
    /// Validate that this delete may be applied to its target operations,
    /// given the set of existing valid chat ops.
    ///
    /// `valid_ops` is the reduced set of all valid chat operations in the topic,
    /// keyed by hash.
    ///
    /// Rules:
    /// - every hash must resolve to a `Message` or `EditMessage` operation,
    /// - the set must be exactly one complete edit chain (one original message,
    ///   every edit of any member included, no gaps, no extras),
    /// - no other delete may already cover any of the hashes,
    /// - the deleter must be the author of every operation in the set,
    /// - the delete must fall within [`DELETE_WINDOW_MICROS`] of the original
    ///   message.
    pub fn validate(&self, valid_ops: &ValidChatOps) -> Result<(), DeleteError> {
        self.check_hashes_form_complete_chain(valid_ops)?;
        self.check_not_already_deleted(valid_ops)?;
        self.check_within_delete_window(valid_ops)?;
        Ok(())
    }

    fn check_hashes_form_complete_chain(
        &self,
        valid_ops: &ValidChatOps,
    ) -> Result<(), DeleteError> {
        if self.hashes.is_empty() {
            return Err(DeleteError::IncompleteChain);
        }

        let mut root = None;
        for hash in &self.hashes {
            let op = valid_ops.get(hash).ok_or(DeleteError::TargetNotFound)?;
            match &op.kind {
                ChatOpKind::Message => {
                    // Exactly one original message per chain.
                    if root.replace(*hash).is_some() {
                        return Err(DeleteError::IncompleteChain);
                    }
                }
                ChatOpKind::Edit(target) => {
                    // No gaps: each edit's target is part of the set too.
                    if !self.hashes.contains(target) {
                        return Err(DeleteError::IncompleteChain);
                    }
                }
                ChatOpKind::Delete(_) | ChatOpKind::Other => {
                    return Err(DeleteError::TargetNotDeletable);
                }
            }
            if op.author != self.deleter {
                return Err(DeleteError::NotAuthor);
            }
        }
        if root.is_none() {
            return Err(DeleteError::IncompleteChain);
        }

        // Complete up to the tip: no valid edit of any member may be left out.
        let leaves_an_edit_out = valid_ops.iter().any(|(hash, op)| {
            matches!(&op.kind, ChatOpKind::Edit(t) if self.hashes.contains(t))
                && !self.hashes.contains(hash)
        });
        if leaves_an_edit_out {
            return Err(DeleteError::IncompleteChain);
        }

        Ok(())
    }

    /// Competing deletes resolve exactly like competing edits: the delete
    /// published earliest (lowest seq_num, hash as tiebreaker) survives; any
    /// later delete covering one of the same operations is invalid. On the
    /// author side (`self_hash` is `None`) any existing delete blocks outright.
    fn check_not_already_deleted(&self, valid_ops: &ValidChatOps) -> Result<(), DeleteError> {
        let self_order_key = self
            .self_hash
            .and_then(|h| valid_ops.get(&h).map(|op| (op.seq_num, h)));
        let already_deleted = valid_ops.iter().any(|(hash, op)| {
            let ChatOpKind::Delete(covered) = &op.kind else {
                return false;
            };
            if covered.is_disjoint(&self.hashes) {
                return false;
            }
            let conflicting_order_key = (op.seq_num, *hash);
            match self_order_key {
                Some(self_order_key) => conflicting_order_key < self_order_key,
                None => true,
            }
        });
        if already_deleted {
            return Err(DeleteError::AlreadyDeleted);
        }
        Ok(())
    }

    fn check_within_delete_window(&self, valid_ops: &ValidChatOps) -> Result<(), DeleteError> {
        let root_timestamp = self
            .hashes
            .iter()
            .find_map(|hash| {
                let op = valid_ops.get(hash)?;
                matches!(op.kind, ChatOpKind::Message).then_some(op.timestamp)
            })
            .ok_or(DeleteError::TargetNotFound)?;
        if self.delete_timestamp.saturating_sub(root_timestamp) > DELETE_WINDOW_MICROS {
            return Err(DeleteError::WindowExpired);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use maplit::btreeset;

    fn device(n: u8) -> DeviceId {
        DeviceId::from(p2panda::SigningKey::from_bytes(&[n; 32]).verifying_key())
    }

    fn hash(n: u8) -> Hash {
        Hash::from_bytes([n; 32])
    }

    fn message(author: DeviceId, timestamp: u64, seq_num: u64) -> ChatOp {
        ChatOp {
            author,
            timestamp,
            seq_num,
            kind: ChatOpKind::Message,
        }
    }

    fn edit(author: DeviceId, timestamp: u64, seq_num: u64, target: Hash) -> ChatOp {
        ChatOp {
            author,
            timestamp,
            seq_num,
            kind: ChatOpKind::Edit(target),
        }
    }

    fn delete(author: DeviceId, timestamp: u64, seq_num: u64, hashes: BTreeSet<Hash>) -> ChatOp {
        ChatOp {
            author,
            timestamp,
            seq_num,
            kind: ChatOpKind::Delete(hashes),
        }
    }

    fn other(author: DeviceId, timestamp: u64, seq_num: u64) -> ChatOp {
        ChatOp {
            author,
            timestamp,
            seq_num,
            kind: ChatOpKind::Other,
        }
    }

    #[test]
    fn collect_chain_of_unedited_message() {
        let alice = device(1);
        let ops = ValidChatOps::new([(hash(1), message(alice, 1000, 0))]);
        assert_eq!(
            collect_deletable_edit_chain(&ops, &hash(1)),
            Ok(btreeset![hash(1)])
        );
    }

    #[test]
    fn collect_chain_walks_edits_to_root() {
        let alice = device(1);
        let ops = ValidChatOps::new([
            (hash(1), message(alice, 1000, 0)),
            (hash(2), edit(alice, 2000, 1, hash(1))),
            (hash(3), edit(alice, 3000, 2, hash(2))),
        ]);
        assert_eq!(
            collect_deletable_edit_chain(&ops, &hash(3)),
            Ok(btreeset![hash(1), hash(2), hash(3)])
        );
    }

    #[test]
    fn collect_chain_rejects_non_tip_target() {
        let alice = device(1);
        let ops = ValidChatOps::new([
            (hash(1), message(alice, 1000, 0)),
            (hash(2), edit(alice, 2000, 1, hash(1))),
        ]);
        // The original message has been edited, so it is not the tip.
        assert_eq!(
            collect_deletable_edit_chain(&ops, &hash(1)),
            Err(DeleteError::NotLatestEdit)
        );
    }

    #[test]
    fn collect_chain_rejects_non_message_target() {
        let alice = device(1);
        let ops = ValidChatOps::new([(hash(1), other(alice, 1000, 0))]);
        assert_eq!(
            collect_deletable_edit_chain(&ops, &hash(1)),
            Err(DeleteError::TargetNotDeletable)
        );
    }

    #[test]
    fn valid_delete_of_unedited_message() {
        let alice = device(1);
        let ops = ValidChatOps::new([(hash(1), message(alice, 1000, 0))]);
        assert_eq!(
            DeleteCandidate {
                hashes: btreeset![hash(1)],
                deleter: alice,
                delete_timestamp: 2000,
                self_hash: None,
            }
            .validate(&ops),
            Ok(())
        );
    }

    #[test]
    fn valid_delete_of_full_chain() {
        let alice = device(1);
        let ops = ValidChatOps::new([
            (hash(1), message(alice, 1000, 0)),
            (hash(2), edit(alice, 2000, 1, hash(1))),
        ]);
        assert_eq!(
            DeleteCandidate {
                hashes: btreeset![hash(1), hash(2)],
                deleter: alice,
                delete_timestamp: 3000,
                self_hash: None,
            }
            .validate(&ops),
            Ok(())
        );
    }

    #[test]
    fn delete_missing_op_is_rejected() {
        let alice = device(1);
        let ops = ValidChatOps::new([(hash(1), message(alice, 1000, 0))]);
        assert_eq!(
            DeleteCandidate {
                hashes: btreeset![hash(9)],
                deleter: alice,
                delete_timestamp: 2000,
                self_hash: None,
            }
            .validate(&ops),
            Err(DeleteError::TargetNotFound)
        );
    }

    #[test]
    fn delete_of_non_message_is_rejected() {
        let alice = device(1);
        let ops = ValidChatOps::new([(hash(1), other(alice, 1000, 0))]);
        assert_eq!(
            DeleteCandidate {
                hashes: btreeset![hash(1)],
                deleter: alice,
                delete_timestamp: 2000,
                self_hash: None,
            }
            .validate(&ops),
            Err(DeleteError::TargetNotDeletable)
        );
    }

    #[test]
    fn delete_leaving_out_an_edit_is_rejected() {
        let alice = device(1);
        let ops = ValidChatOps::new([
            (hash(1), message(alice, 1000, 0)),
            (hash(2), edit(alice, 2000, 1, hash(1))),
        ]);
        // The set covers the original but not its edit.
        assert_eq!(
            DeleteCandidate {
                hashes: btreeset![hash(1)],
                deleter: alice,
                delete_timestamp: 3000,
                self_hash: None,
            }
            .validate(&ops),
            Err(DeleteError::IncompleteChain)
        );
    }

    #[test]
    fn delete_leaving_out_the_root_is_rejected() {
        let alice = device(1);
        let ops = ValidChatOps::new([
            (hash(1), message(alice, 1000, 0)),
            (hash(2), edit(alice, 2000, 1, hash(1))),
        ]);
        // The set covers the edit but not the original it points at.
        assert_eq!(
            DeleteCandidate {
                hashes: btreeset![hash(2)],
                deleter: alice,
                delete_timestamp: 3000,
                self_hash: None,
            }
            .validate(&ops),
            Err(DeleteError::IncompleteChain)
        );
    }

    #[test]
    fn delete_spanning_two_chains_is_rejected() {
        let alice = device(1);
        let ops = ValidChatOps::new([
            (hash(1), message(alice, 1000, 0)),
            (hash(2), message(alice, 1000, 1)),
        ]);
        assert_eq!(
            DeleteCandidate {
                hashes: btreeset![hash(1), hash(2)],
                deleter: alice,
                delete_timestamp: 2000,
                self_hash: None,
            }
            .validate(&ops),
            Err(DeleteError::IncompleteChain)
        );
    }

    #[test]
    fn delete_by_non_author_is_rejected() {
        let alice = device(1);
        let bobbi = device(2);
        let ops = ValidChatOps::new([(hash(1), message(alice, 1000, 0))]);
        assert_eq!(
            DeleteCandidate {
                hashes: btreeset![hash(1)],
                deleter: bobbi,
                delete_timestamp: 2000,
                self_hash: None,
            }
            .validate(&ops),
            Err(DeleteError::NotAuthor)
        );
    }

    #[test]
    fn double_delete_is_rejected() {
        let alice = device(1);
        let ops = ValidChatOps::new([
            (hash(1), message(alice, 1000, 0)),
            (hash(3), delete(alice, 2000, 1, btreeset![hash(1)])),
        ]);
        assert_eq!(
            DeleteCandidate {
                hashes: btreeset![hash(1)],
                deleter: alice,
                delete_timestamp: 3000,
                self_hash: None,
            }
            .validate(&ops),
            Err(DeleteError::AlreadyDeleted)
        );
    }

    #[test]
    fn competing_deletes_earliest_seq_num_wins() {
        let alice = device(1);
        let ops = ValidChatOps::new([
            (hash(1), message(alice, 1000, 0)),
            (hash(2), delete(alice, 2000, 2, btreeset![hash(1)])),
            (hash(3), delete(alice, 2000, 1, btreeset![hash(1)])),
        ]);
        assert_eq!(
            DeleteCandidate {
                hashes: btreeset![hash(1)],
                deleter: alice,
                delete_timestamp: 2000,
                self_hash: Some(hash(3)),
            }
            .validate(&ops),
            Ok(())
        );
        assert_eq!(
            DeleteCandidate {
                hashes: btreeset![hash(1)],
                deleter: alice,
                delete_timestamp: 2000,
                self_hash: Some(hash(2)),
            }
            .validate(&ops),
            Err(DeleteError::AlreadyDeleted)
        );
    }

    #[test]
    fn window_measured_from_root_message() {
        let alice = device(1);
        let ops = ValidChatOps::new([
            (hash(1), message(alice, 1000, 0)),
            (hash(2), edit(alice, 2000, 1, hash(1))),
        ]);
        let set = btreeset![hash(1), hash(2)];
        assert_eq!(
            DeleteCandidate {
                hashes: set.clone(),
                deleter: alice,
                delete_timestamp: 1000 + DELETE_WINDOW_MICROS,
                self_hash: None,
            }
            .validate(&ops),
            Ok(())
        );
        assert_eq!(
            DeleteCandidate {
                hashes: set.clone(),
                deleter: alice,
                delete_timestamp: 1000 + DELETE_WINDOW_MICROS + 1,
                self_hash: None,
            }
            .validate(&ops),
            Err(DeleteError::WindowExpired)
        );
    }
}
