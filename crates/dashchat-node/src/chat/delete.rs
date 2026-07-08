use std::collections::{BTreeSet, HashMap};

use p2panda::Hash;
use serde::Serialize;
use thiserror::Error;

use crate::DeviceId;

use super::edit::{ChatOp, ChatOpKind, EDIT_WINDOW_MICROS};

/// The window during which a message may be deleted for everyone, measured from
/// the original message timestamp. Matches the edit window (24h in µs).
pub const DELETE_WINDOW_MICROS: u64 = EDIT_WINDOW_MICROS;

/// Why a delete operation is not allowed to be applied.
///
/// The same rules are enforced on the author's side (as a hard error before
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

/// Collect the full edit chain ending at `target`: the original message plus
/// every edit between it and `target`, inclusive. `target` must be the most
/// recent edit of its chain (or the message itself when unedited) — deleting
/// anything but the tip is rejected with [`DeleteError::NotLatestEdit`].
///
/// This is the author-side helper that builds the hash set for a
/// `DeleteMessage` payload.
pub fn collect_edit_chain(
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

/// Validate that a delete may be applied to the operations in `hashes`.
///
/// `valid_ops` is the reduced set of all valid chat operations in the topic,
/// keyed by hash. `deleter` and `delete_timestamp` describe the delete itself.
/// `self_hash` is the hash of the delete operation when validating a delete
/// that is already in `valid_ops` (the receiving side); it is `None` when an
/// author validates before publishing.
///
/// Rules:
/// - every hash must resolve to a `Message` or `EditMessage` operation,
/// - the set must be exactly one complete edit chain (one original message,
///   every edit of any member included, no gaps, no extras),
/// - no other delete may already cover any of the hashes,
/// - the deleter must be the author of every operation in the set,
/// - the delete must fall within [`DELETE_WINDOW_MICROS`] of the original
///   message.
pub fn validate_delete(
    valid_ops: &HashMap<Hash, ChatOp>,
    hashes: &BTreeSet<Hash>,
    deleter: DeviceId,
    delete_timestamp: u64,
    self_hash: Option<&Hash>,
) -> Result<(), DeleteError> {
    if hashes.is_empty() {
        return Err(DeleteError::IncompleteChain);
    }

    let mut root = None;
    for hash in hashes {
        let op = valid_ops.get(hash).ok_or(DeleteError::TargetNotFound)?;
        match &op.kind {
            ChatOpKind::Message => {
                // Exactly one original message per chain.
                if root.replace((*hash, op)).is_some() {
                    return Err(DeleteError::IncompleteChain);
                }
            }
            ChatOpKind::Edit(target) => {
                // No gaps: each edit's target is part of the set too.
                if !hashes.contains(target) {
                    return Err(DeleteError::IncompleteChain);
                }
            }
            ChatOpKind::Delete(_) | ChatOpKind::Other => {
                return Err(DeleteError::TargetNotDeletable);
            }
        }
        if op.author != deleter {
            return Err(DeleteError::NotAuthor);
        }
    }
    let Some((_, root)) = root else {
        return Err(DeleteError::IncompleteChain);
    };

    // Complete up to the tip: no valid edit of any member may be left out.
    let leaves_an_edit_out = valid_ops.iter().any(|(hash, op)| {
        matches!(&op.kind, ChatOpKind::Edit(t) if hashes.contains(t)) && !hashes.contains(hash)
    });
    if leaves_an_edit_out {
        return Err(DeleteError::IncompleteChain);
    }

    // Competing deletes resolve exactly like competing edits: the delete
    // published earliest (lowest seq_num, hash as tiebreaker) survives; any
    // later delete covering one of the same operations is invalid. On the
    // author side (`self_hash` is `None`) any existing delete blocks outright.
    let self_order = self_hash.and_then(|h| valid_ops.get(h).map(|op| (op.seq_num, h)));
    let already_deleted = valid_ops.iter().any(|(hash, op)| {
        let ChatOpKind::Delete(covered) = &op.kind else {
            return false;
        };
        if covered.is_disjoint(hashes) {
            return false;
        }
        match self_order {
            Some(self_order) => (op.seq_num, hash) < self_order,
            None => true,
        }
    });
    if already_deleted {
        return Err(DeleteError::AlreadyDeleted);
    }

    if delete_timestamp.saturating_sub(root.timestamp) > DELETE_WINDOW_MICROS {
        return Err(DeleteError::WindowExpired);
    }

    Ok(())
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
        let ops = HashMap::from([(hash(1), message(alice, 1000, 0))]);
        assert_eq!(collect_edit_chain(&ops, &hash(1)), Ok(btreeset![hash(1)]));
    }

    #[test]
    fn collect_chain_walks_edits_to_root() {
        let alice = device(1);
        let ops = HashMap::from([
            (hash(1), message(alice, 1000, 0)),
            (hash(2), edit(alice, 2000, 1, hash(1))),
            (hash(3), edit(alice, 3000, 2, hash(2))),
        ]);
        assert_eq!(
            collect_edit_chain(&ops, &hash(3)),
            Ok(btreeset![hash(1), hash(2), hash(3)])
        );
    }

    #[test]
    fn collect_chain_rejects_non_tip_target() {
        let alice = device(1);
        let ops = HashMap::from([
            (hash(1), message(alice, 1000, 0)),
            (hash(2), edit(alice, 2000, 1, hash(1))),
        ]);
        // The original message has been edited, so it is not the tip.
        assert_eq!(
            collect_edit_chain(&ops, &hash(1)),
            Err(DeleteError::NotLatestEdit)
        );
    }

    #[test]
    fn collect_chain_rejects_non_message_target() {
        let alice = device(1);
        let ops = HashMap::from([(hash(1), other(alice, 1000, 0))]);
        assert_eq!(
            collect_edit_chain(&ops, &hash(1)),
            Err(DeleteError::TargetNotDeletable)
        );
    }

    #[test]
    fn valid_delete_of_unedited_message() {
        let alice = device(1);
        let ops = HashMap::from([(hash(1), message(alice, 1000, 0))]);
        assert_eq!(
            validate_delete(&ops, &btreeset![hash(1)], alice, 2000, None),
            Ok(())
        );
    }

    #[test]
    fn valid_delete_of_full_chain() {
        let alice = device(1);
        let ops = HashMap::from([
            (hash(1), message(alice, 1000, 0)),
            (hash(2), edit(alice, 2000, 1, hash(1))),
        ]);
        assert_eq!(
            validate_delete(&ops, &btreeset![hash(1), hash(2)], alice, 3000, None),
            Ok(())
        );
    }

    #[test]
    fn delete_missing_op_is_rejected() {
        let alice = device(1);
        let ops = HashMap::from([(hash(1), message(alice, 1000, 0))]);
        assert_eq!(
            validate_delete(&ops, &btreeset![hash(9)], alice, 2000, None),
            Err(DeleteError::TargetNotFound)
        );
    }

    #[test]
    fn delete_of_non_message_is_rejected() {
        let alice = device(1);
        let ops = HashMap::from([(hash(1), other(alice, 1000, 0))]);
        assert_eq!(
            validate_delete(&ops, &btreeset![hash(1)], alice, 2000, None),
            Err(DeleteError::TargetNotDeletable)
        );
    }

    #[test]
    fn delete_leaving_out_an_edit_is_rejected() {
        let alice = device(1);
        let ops = HashMap::from([
            (hash(1), message(alice, 1000, 0)),
            (hash(2), edit(alice, 2000, 1, hash(1))),
        ]);
        // The set covers the original but not its edit.
        assert_eq!(
            validate_delete(&ops, &btreeset![hash(1)], alice, 3000, None),
            Err(DeleteError::IncompleteChain)
        );
    }

    #[test]
    fn delete_leaving_out_the_root_is_rejected() {
        let alice = device(1);
        let ops = HashMap::from([
            (hash(1), message(alice, 1000, 0)),
            (hash(2), edit(alice, 2000, 1, hash(1))),
        ]);
        // The set covers the edit but not the original it points at.
        assert_eq!(
            validate_delete(&ops, &btreeset![hash(2)], alice, 3000, None),
            Err(DeleteError::IncompleteChain)
        );
    }

    #[test]
    fn delete_spanning_two_chains_is_rejected() {
        let alice = device(1);
        let ops = HashMap::from([
            (hash(1), message(alice, 1000, 0)),
            (hash(2), message(alice, 1000, 1)),
        ]);
        assert_eq!(
            validate_delete(&ops, &btreeset![hash(1), hash(2)], alice, 2000, None),
            Err(DeleteError::IncompleteChain)
        );
    }

    #[test]
    fn delete_by_non_author_is_rejected() {
        let alice = device(1);
        let bobbi = device(2);
        let ops = HashMap::from([(hash(1), message(alice, 1000, 0))]);
        assert_eq!(
            validate_delete(&ops, &btreeset![hash(1)], bobbi, 2000, None),
            Err(DeleteError::NotAuthor)
        );
    }

    #[test]
    fn double_delete_is_rejected() {
        let alice = device(1);
        let ops = HashMap::from([
            (hash(1), message(alice, 1000, 0)),
            (hash(3), delete(alice, 2000, 1, btreeset![hash(1)])),
        ]);
        assert_eq!(
            validate_delete(&ops, &btreeset![hash(1)], alice, 3000, None),
            Err(DeleteError::AlreadyDeleted)
        );
    }

    #[test]
    fn competing_deletes_earliest_seq_num_wins() {
        let alice = device(1);
        let ops = HashMap::from([
            (hash(1), message(alice, 1000, 0)),
            (hash(2), delete(alice, 2000, 2, btreeset![hash(1)])),
            (hash(3), delete(alice, 2000, 1, btreeset![hash(1)])),
        ]);
        assert_eq!(
            validate_delete(&ops, &btreeset![hash(1)], alice, 2000, Some(&hash(3))),
            Ok(())
        );
        assert_eq!(
            validate_delete(&ops, &btreeset![hash(1)], alice, 2000, Some(&hash(2))),
            Err(DeleteError::AlreadyDeleted)
        );
    }

    #[test]
    fn window_measured_from_root_message() {
        let alice = device(1);
        let ops = HashMap::from([
            (hash(1), message(alice, 1000, 0)),
            (hash(2), edit(alice, 2000, 1, hash(1))),
        ]);
        let set = btreeset![hash(1), hash(2)];
        assert_eq!(
            validate_delete(&ops, &set, alice, 1000 + DELETE_WINDOW_MICROS, None),
            Ok(())
        );
        assert_eq!(
            validate_delete(&ops, &set, alice, 1000 + DELETE_WINDOW_MICROS + 1, None),
            Err(DeleteError::WindowExpired)
        );
    }
}
