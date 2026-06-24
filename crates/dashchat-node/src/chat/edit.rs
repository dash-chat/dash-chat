use std::collections::HashMap;

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

/// The kind of a chat operation, reduced to what edit validation cares about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatOpKind {
    /// An original `ChatPayload::Message`.
    Message,
    /// A `ChatPayload::EditMessage` pointing at the operation it edits.
    Edit(Hash),
    /// Any other chat payload (reaction, group info, …) which cannot be edited.
    Other,
}

/// A chat operation reduced to the fields edit validation needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatOp {
    pub author: DeviceId,
    /// Microseconds since the UNIX epoch (the operation header timestamp).
    pub timestamp: u64,
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
/// edit operation when validating a received edit (so it is excluded from the
/// "already edited" scan); it is `None` when an author validates before
/// publishing.
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
        ChatOpKind::Other => return Err(EditError::TargetNotEditable),
    }

    // TODO: this is only a same-device check.
    // If we want editing across devices, we need to check against AgentId, which is more complicated.
    if editor != target.author {
        return Err(EditError::NotAuthor);
    }

    let already_edited = valid_ops.iter().any(|(hash, op)| {
        Some(hash) != self_hash && matches!(&op.kind, ChatOpKind::Edit(t) if t == edit_hash)
    });
    if already_edited {
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
            ChatOpKind::Other => return None,
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

    fn message(author: DeviceId, timestamp: u64) -> ChatOp {
        ChatOp {
            author,
            timestamp,
            kind: ChatOpKind::Message,
        }
    }

    fn edit(author: DeviceId, timestamp: u64, target: Hash) -> ChatOp {
        ChatOp {
            author,
            timestamp,
            kind: ChatOpKind::Edit(target),
        }
    }

    fn other(author: DeviceId, timestamp: u64) -> ChatOp {
        ChatOp {
            author,
            timestamp,
            kind: ChatOpKind::Other,
        }
    }

    #[test]
    fn valid_first_edit() {
        let alice = device(1);
        let ops = HashMap::from([(hash(1), message(alice, 1000))]);
        assert_eq!(validate_edit(&ops, &hash(1), alice, 2000, None), Ok(()));
    }

    #[test]
    fn valid_chained_edit() {
        let alice = device(1);
        let ops = HashMap::from([
            (hash(1), message(alice, 1000)),
            (hash(2), edit(alice, 2000, hash(1))),
        ]);
        // Editing the edit (hash 2) extends the linear chain.
        assert_eq!(validate_edit(&ops, &hash(2), alice, 3000, None), Ok(()));
    }

    #[test]
    fn target_not_found() {
        let alice = device(1);
        let ops = HashMap::from([(hash(1), message(alice, 1000))]);
        assert_eq!(
            validate_edit(&ops, &hash(9), alice, 2000, None),
            Err(EditError::TargetNotFound)
        );
    }

    #[test]
    fn target_not_editable() {
        let alice = device(1);
        // hash 1 is a reaction / group-info / etc.
        let ops = HashMap::from([(hash(1), other(alice, 1000))]);
        assert_eq!(
            validate_edit(&ops, &hash(1), alice, 2000, None),
            Err(EditError::TargetNotEditable)
        );
    }

    #[test]
    fn not_author() {
        let alice = device(1);
        let bobbi = device(2);
        let ops = HashMap::from([(hash(1), message(alice, 1000))]);
        assert_eq!(
            validate_edit(&ops, &hash(1), bobbi, 2000, None),
            Err(EditError::NotAuthor)
        );
    }

    #[test]
    fn already_edited_is_rejected() {
        let alice = device(1);
        let ops = HashMap::from([
            (hash(1), message(alice, 1000)),
            (hash(2), edit(alice, 2000, hash(1))),
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
            (hash(1), message(alice, 1000)),
            (hash(2), edit(alice, 2000, hash(1))),
        ]);
        // Validating the existing edit (hash 2) against its own target must not
        // see itself as a competing edit.
        assert_eq!(
            validate_edit(&ops, &hash(1), alice, 2000, Some(&hash(2))),
            Ok(())
        );
    }

    #[test]
    fn window_measured_from_root_message() {
        let alice = device(1);
        let ops = HashMap::from([
            (hash(1), message(alice, 1000)),
            (hash(2), edit(alice, 1000 + EDIT_WINDOW_MICROS, hash(1))),
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
        let ops = HashMap::from([(hash(1), message(alice, 1000))]);
        assert_eq!(
            validate_edit(&ops, &hash(1), alice, 1000 + EDIT_WINDOW_MICROS + 1, None),
            Err(EditError::WindowExpired)
        );
    }
}
