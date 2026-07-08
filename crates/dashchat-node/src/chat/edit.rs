use std::collections::{BTreeSet, HashMap};

use p2panda::Hash;
use serde::Serialize;
use thiserror::Error;

use crate::DeviceId;

/// The window during which a message may be edited, measured from the original
/// message timestamp. p2panda header timestamps are microseconds since the UNIX
/// epoch, so this is 24 hours expressed in microseconds.
pub const EDIT_WINDOW_MICROS: u64 = 24 * 60 * 60 * 1_000_000;

/// Why an edit operation is not allowed to be applied to its target.
///
/// The same rules are enforced on the author's side (as a hard error before
/// publishing) and on the receiving side (the edit is ignored with a warning).
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize)]
#[serde(tag = "kind", content = "message")]
pub enum EditError {
    #[error("the message being edited could not be found in this chat")]
    TargetNotFound,

    #[error("only text messages can be edited")]
    TargetNotEditable,

    #[error("a message can only be edited by its original author")]
    NotAuthor,

    #[error("this message has already been edited; edits must form a linear chain")]
    AlreadyEdited,

    #[error("the edit window for this message has expired")]
    WindowExpired,
}

/// The kind of a chat operation, reduced to what edit/delete validation cares about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatOpKind {
    /// An original `ChatPayload::Message`.
    Message,
    /// A `ChatPayload::EditMessage` pointing at the operation it edits.
    Edit(Hash),
    /// A `ChatPayload::DeleteMessage` carrying the full edit chain it deletes.
    Delete(BTreeSet<Hash>),
    /// Any other chat payload (reaction, group info, …) which cannot be edited.
    Other,
}

/// A chat operation reduced to the fields edit validation needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatOp {
    pub author: DeviceId,
    /// Microseconds since the UNIX epoch (the operation header timestamp).
    pub timestamp: u64,
    /// Position in the author's append-only log; monotonic in publish order.
    pub seq_num: u64,
    pub kind: ChatOpKind,
}

/// A received edit that passed validation, exposed for tests.
#[cfg(any(test, feature = "testing"))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidEdit {
    /// Hash of the edit operation itself.
    pub op_hash: Hash,
    /// Hash of the operation it edits.
    pub target: Hash,
    /// The new text content.
    pub text: String,
}

/// Validate that an edit may be applied to its target message.
///
/// `ops` is the set of all chat operations in the topic, keyed by hash.
/// `edit_hash` is the operation the edit points at. `editor` and
/// `edit_timestamp` describe the edit itself. `self_hash` is the hash of the
/// edit operation when validating an edit that is already in `ops` (the
/// receiving/reduction side); it identifies which competing edit this call is
/// judging so exactly one of them survives. It is `None` when an author
/// validates before publishing, where any existing edit of the target is
/// rejected.
pub fn validate_edit(
    valid_ops: &HashMap<Hash, ChatOp>,
    edit_hash: &Hash,
    editor: DeviceId,
    edit_timestamp: u64,
    self_hash: Option<&Hash>,
) -> Result<(), EditError> {
    let target = valid_ops.get(edit_hash).ok_or(EditError::TargetNotFound)?;

    match target.kind {
        ChatOpKind::Message | ChatOpKind::Edit(_) => {}
        ChatOpKind::Delete(_) | ChatOpKind::Other => return Err(EditError::TargetNotEditable),
    }

    // TODO: this is only a same-device check.
    // If we want editing across devices, we need to check against AgentId, which is more complicated.
    if editor != target.author {
        return Err(EditError::NotAuthor);
    }

    // Edits must form a linear chain: a target may be edited at most once. Two
    // edits of the same target can still arise honestly — e.g. a UI that
    // "bounces" repeated edit clicks before the first is committed — so instead
    // of dropping all of them we keep exactly one: the edit published earliest.
    // `seq_num` on a single author's append-only log is monotonic in publish
    // order, so the lowest `seq_num` was written first and any later edit had
    // knowledge of it.
    //
    // The op hash only breaks the tie in the pathological case
    // of a forked log reusing a `seq_num`, keeping every peer's choice
    // deterministic. On the receiving/reduction side (`self_hash` is set) this
    // edit is already in `valid_ops` and the `(seq_num, hash)` key excludes it
    // from itself; on the author side (`self_hash` is `None`, before the op has
    // a hash) any existing edit blocks publishing outright.
    let self_order = self_hash.and_then(|h| valid_ops.get(h).map(|op| (op.seq_num, h)));
    let superseded = valid_ops.iter().any(|(hash, op)| {
        if !matches!(&op.kind, ChatOpKind::Edit(t) if t == edit_hash) {
            return false;
        }
        match self_order {
            Some(self_order) => (op.seq_num, hash) < self_order,
            None => true,
        }
    });
    if superseded {
        return Err(EditError::AlreadyEdited);
    }

    let root_timestamp =
        root_message_timestamp(valid_ops, edit_hash).ok_or(EditError::TargetNotFound)?;
    if edit_timestamp.saturating_sub(root_timestamp) > EDIT_WINDOW_MICROS {
        return Err(EditError::WindowExpired);
    }

    Ok(())
}

/// Walk the edit chain back from `start` to the original message and return its
/// timestamp. Returns `None` if the chain is broken (a link is missing or
/// reaches a non-editable operation) or cyclic.
fn root_message_timestamp(ops: &HashMap<Hash, ChatOp>, start: &Hash) -> Option<u64> {
    let mut current = start;
    for _ in 0..ops.len() + 1 {
        let op = ops.get(current)?;
        match &op.kind {
            ChatOpKind::Message => return Some(op.timestamp),
            ChatOpKind::Edit(target) => current = target,
            ChatOpKind::Delete(_) | ChatOpKind::Other => return None,
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn other(author: DeviceId, timestamp: u64, seq_num: u64) -> ChatOp {
        ChatOp {
            author,
            timestamp,
            seq_num,
            kind: ChatOpKind::Other,
        }
    }

    #[test]
    fn valid_first_edit() {
        let alice = device(1);
        let ops = HashMap::from([(hash(1), message(alice, 1000, 0))]);
        assert_eq!(validate_edit(&ops, &hash(1), alice, 2000, None), Ok(()));
    }

    #[test]
    fn valid_chained_edit() {
        let alice = device(1);
        let ops = HashMap::from([
            (hash(1), message(alice, 1000, 0)),
            (hash(2), edit(alice, 2000, 1, hash(1))),
        ]);
        // Editing the edit (hash 2) extends the linear chain.
        assert_eq!(validate_edit(&ops, &hash(2), alice, 3000, None), Ok(()));
    }

    #[test]
    fn target_not_found() {
        let alice = device(1);
        let ops = HashMap::from([(hash(1), message(alice, 1000, 0))]);
        assert_eq!(
            validate_edit(&ops, &hash(9), alice, 2000, None),
            Err(EditError::TargetNotFound)
        );
    }

    #[test]
    fn target_not_editable() {
        let alice = device(1);
        // hash 1 is a reaction / group-info / etc.
        let ops = HashMap::from([(hash(1), other(alice, 1000, 0))]);
        assert_eq!(
            validate_edit(&ops, &hash(1), alice, 2000, None),
            Err(EditError::TargetNotEditable)
        );
    }

    #[test]
    fn not_author() {
        let alice = device(1);
        let bobbi = device(2);
        let ops = HashMap::from([(hash(1), message(alice, 1000, 0))]);
        assert_eq!(
            validate_edit(&ops, &hash(1), bobbi, 2000, None),
            Err(EditError::NotAuthor)
        );
    }

    #[test]
    fn already_edited_is_rejected() {
        let alice = device(1);
        let ops = HashMap::from([
            (hash(1), message(alice, 1000, 0)),
            (hash(2), edit(alice, 2000, 1, hash(1))),
        ]);
        // A second edit of hash 1 would form a tree, not a chain.
        assert_eq!(
            validate_edit(&ops, &hash(1), alice, 2500, None),
            Err(EditError::AlreadyEdited)
        );
    }

    #[test]
    fn self_hash_excluded_from_already_edited_scan() {
        let alice = device(1);
        let ops = HashMap::from([
            (hash(1), message(alice, 1000, 0)),
            (hash(2), edit(alice, 2000, 1, hash(1))),
        ]);
        // Validating the existing edit (hash 2) against its own target must not
        // see itself as a competing edit.
        assert_eq!(
            validate_edit(&ops, &hash(1), alice, 2000, Some(&hash(2))),
            Ok(())
        );
    }

    #[test]
    fn competing_edits_earliest_seq_num_wins() {
        let alice = device(1);
        // hash(2) and hash(3) both edit the same original hash(1) — e.g. a UI
        // that bounced two edit clicks before the first was committed. The one
        // published earlier (lower seq_num) survives; the later one loses.
        let ops = HashMap::from([
            (hash(1), message(alice, 1000, 0)),
            (hash(2), edit(alice, 2000, 2, hash(1))),
            (hash(3), edit(alice, 2000, 1, hash(1))),
        ]);

        // hash(3) has the lower seq_num, so it wins
        assert_eq!(
            validate_edit(&ops, &hash(1), alice, 2000, Some(&hash(3))),
            Ok(())
        );
        assert_eq!(
            validate_edit(&ops, &hash(1), alice, 2000, Some(&hash(2))),
            Err(EditError::AlreadyEdited)
        );
    }

    #[test]
    fn competing_edits_same_seq_num_break_ties_by_hash() {
        let alice = device(1);
        // A forked log can reuse a seq_num; the lower op hash then wins so every
        // peer converges on the same survivor.
        let ops = HashMap::from([
            (hash(1), message(alice, 1000, 0)),
            (hash(2), edit(alice, 2000, 1, hash(1))),
            (hash(3), edit(alice, 2000, 1, hash(1))),
        ]);
        assert_eq!(
            validate_edit(&ops, &hash(1), alice, 2000, Some(&hash(2))),
            Ok(())
        );
        assert_eq!(
            validate_edit(&ops, &hash(1), alice, 2000, Some(&hash(3))),
            Err(EditError::AlreadyEdited)
        );
    }

    #[test]
    fn window_measured_from_root_message() {
        let alice = device(1);
        let ops = HashMap::from([
            (hash(1), message(alice, 1000, 0)),
            (hash(2), edit(alice, 1000 + EDIT_WINDOW_MICROS, 1, hash(1))),
        ]);
        // Editing the chained edit just within the window from the root is ok.
        assert_eq!(
            validate_edit(&ops, &hash(2), alice, 1000 + EDIT_WINDOW_MICROS, None),
            Ok(())
        );
        // Just past the window from the root is rejected, even though the direct
        // target (hash 2) is recent.
        assert_eq!(
            validate_edit(&ops, &hash(2), alice, 1000 + EDIT_WINDOW_MICROS + 1, None),
            Err(EditError::WindowExpired)
        );
    }

    #[test]
    fn window_expired_on_first_edit() {
        let alice = device(1);
        let ops = HashMap::from([(hash(1), message(alice, 1000, 0))]);
        assert_eq!(
            validate_edit(&ops, &hash(1), alice, 1000 + EDIT_WINDOW_MICROS + 1, None),
            Err(EditError::WindowExpired)
        );
    }
}
