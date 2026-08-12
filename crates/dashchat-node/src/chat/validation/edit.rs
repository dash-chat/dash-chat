use p2panda::Hash;
use serde::Serialize;
use thiserror::Error;

use crate::DeviceId;

use super::{ChatOpKind, ValidChatOps};

/// The window during which a message may be edited, measured from the original
/// message timestamp. p2panda header timestamps are microseconds since the UNIX
/// epoch, so this is 24 hours expressed in microseconds.
pub const EDIT_WINDOW_MICROS: u64 = 24 * 60 * 60 * 1_000_000;

/// Why an edit operation is considered invalid
/// (not allowed to be applied to its target).
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

/// A candidate edit to be validated against its target message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditCandidate {
    /// The operation the edit points at.
    pub target: Hash,
    pub editor: DeviceId,
    pub timestamp: u64,
    /// When an author is about to publish an edit op, `self_hash` is `None`
    /// (we don't even bother to construct the op until we know it's valid).
    /// When a receiver is validating an edit op, published by someone else,
    /// then `self_hash` is the hash of that edit operation itself.
    /// The `Option` serves as a distinction between these two cases,
    /// and when validating as a receiver, the hash can be used as a tiebreaker
    /// in case of a forked log.
    pub self_hash: Option<Hash>,
}

impl EditCandidate {
    /// Validate that this edit may be applied to its target message,
    /// given the set of existing valid chat ops.
    pub fn validate(&self, valid_ops: &ValidChatOps) -> Result<(), EditError> {
        self.check_target_editable(valid_ops)?;
        self.check_editor_is_author(valid_ops)?;
        self.check_not_conflicting(valid_ops)?;
        self.check_within_edit_window(valid_ops)?;
        Ok(())
    }

    fn check_target_editable(&self, valid_ops: &ValidChatOps) -> Result<(), EditError> {
        let target = valid_ops
            .get(&self.target)
            .ok_or(EditError::TargetNotFound)?;
        match target.kind {
            ChatOpKind::Message | ChatOpKind::Edit(_) => Ok(()),
            ChatOpKind::Delete(_) | ChatOpKind::Other => Err(EditError::TargetNotEditable),
        }
    }

    // TODO: this is only a same-device check.
    // If we want editing across devices, we need to check against AgentId, which is more complicated.
    fn check_editor_is_author(&self, valid_ops: &ValidChatOps) -> Result<(), EditError> {
        let target = valid_ops
            .get(&self.target)
            .ok_or(EditError::TargetNotFound)?;
        if self.editor != target.author {
            return Err(EditError::NotAuthor);
        }
        Ok(())
    }

    /// Edits must form a linear chain: a target may be edited at most once. Two
    /// edits of the same target can still arise honestly — e.g. a UI that
    /// "bounces" repeated edit clicks before the first is committed — so
    /// instead of dropping all of them we keep exactly one: the edit published
    /// earliest. `seq_num` on a single author's append-only log is monotonic in
    /// publish order, so the lowest `seq_num` was written first and any later
    /// edit had knowledge of it.
    ///
    /// The op hash only breaks the tie in the pathological case of a forked log
    /// reusing a `seq_num`, keeping every peer's choice deterministic. On the
    /// receiving/reduction side (`self_hash` is set) this edit is already in the
    /// set and the `(seq_num, hash)` key excludes it from itself; on the author
    /// side (`self_hash` is `None`, before the op has a hash) any existing edit
    /// blocks publishing outright.
    fn check_not_conflicting(&self, valid_ops: &ValidChatOps) -> Result<(), EditError> {
        // The order key for this candidate, to be compared against
        // the order key of any potentially conflicting edits.
        let self_order_key = self
            .self_hash
            .and_then(|h| valid_ops.get(&h).map(|op| (op.seq_num, h)));
        let invalid = valid_ops.iter().any(|(hash, op)| {
            let conflicting = matches!(&op.kind, ChatOpKind::Edit(t) if t == &self.target);
            if !conflicting {
                return false;
            }
            let conflicting_order_key = (op.seq_num, *hash);
            match self_order_key {
                // If there is an existing conflicting edit with a lower order
                // (lower seq num with hash tiebreaker), then this edit is invalid.
                Some(self_order) => conflicting_order_key < self_order,
                // If there is no self_hash, then there is no self_order.
                // Any existing conflicting edit is necessarily newer than this one,
                // therefore this edit is invalid.
                None => true,
            }
        });
        if invalid {
            return Err(EditError::AlreadyEdited);
        }
        Ok(())
    }

    fn check_within_edit_window(&self, valid_ops: &ValidChatOps) -> Result<(), EditError> {
        let root_timestamp = valid_ops
            .root_message_timestamp_for_edit_chain(&self.target)
            .ok_or(EditError::TargetNotFound)?;
        if self.timestamp.saturating_sub(root_timestamp) > EDIT_WINDOW_MICROS {
            return Err(EditError::WindowExpired);
        }
        Ok(())
    }
}

impl ValidChatOps {
    /// Build the [`EditCandidate`] for an edit already stored under `hash`, for
    /// re-validating it against the rest of the set. `None` if `hash` is absent
    /// or is not an edit.
    #[allow(unused)]
    fn edit_candidate(&self, hash: &Hash) -> Option<EditCandidate> {
        let op = self.get(hash)?;
        let ChatOpKind::Edit(target) = op.kind else {
            return None;
        };
        Some(EditCandidate {
            target,
            editor: op.author,
            timestamp: op.timestamp,
            self_hash: Some(*hash),
        })
    }

    /// Walk the edit chain back from `start` to the original message and return
    /// its timestamp. Returns `None` if the chain is broken (a link is missing
    /// or reaches a non-editable operation) or cyclic.
    fn root_message_timestamp_for_edit_chain(&self, start: &Hash) -> Option<u64> {
        let mut current = start;
        for _ in 0..self.len() + 1 {
            let op = self.get(current)?;
            match &op.kind {
                ChatOpKind::Message => return Some(op.timestamp),
                ChatOpKind::Edit(target) => current = target,
                ChatOpKind::Delete(_) | ChatOpKind::Other => return None,
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::super::test_common::*;
    use super::*;

    #[test]
    fn valid_first_edit() {
        let alice = device(1);
        let ops = ValidChatOps::new([(hash(1), message(alice, 1000, 0))]);
        assert_eq!(
            EditCandidate {
                target: hash(1),
                editor: alice,
                timestamp: 2000,
                self_hash: None,
            }
            .validate(&ops),
            Ok(())
        );
    }

    #[test]
    fn valid_chained_edit() {
        let alice = device(1);
        let ops = ValidChatOps::new([
            (hash(1), message(alice, 1000, 0)),
            (hash(2), edit(alice, 2000, 1, hash(1))),
        ]);
        // Editing the edit (hash 2) extends the linear chain.
        assert_eq!(
            EditCandidate {
                target: hash(2),
                editor: alice,
                timestamp: 3000,
                self_hash: None,
            }
            .validate(&ops),
            Ok(())
        );
    }

    #[test]
    fn target_not_found() {
        let alice = device(1);
        let ops = ValidChatOps::new([(hash(1), message(alice, 1000, 0))]);
        assert_eq!(
            EditCandidate {
                target: hash(9),
                editor: alice,
                timestamp: 2000,
                self_hash: None,
            }
            .validate(&ops),
            Err(EditError::TargetNotFound)
        );
    }

    #[test]
    fn target_not_editable() {
        let alice = device(1);
        // hash 1 is a reaction / group-info / etc.
        let ops = ValidChatOps::new([(hash(1), other(alice, 1000, 0))]);
        assert_eq!(
            EditCandidate {
                target: hash(1),
                editor: alice,
                timestamp: 2000,
                self_hash: None,
            }
            .validate(&ops),
            Err(EditError::TargetNotEditable)
        );
    }

    #[test]
    fn not_author() {
        let alice = device(1);
        let bobbi = device(2);
        let ops = ValidChatOps::new([(hash(1), message(alice, 1000, 0))]);
        assert_eq!(
            EditCandidate {
                target: hash(1),
                editor: bobbi,
                timestamp: 2000,
                self_hash: None,
            }
            .validate(&ops),
            Err(EditError::NotAuthor)
        );
    }

    #[test]
    fn already_edited_is_rejected() {
        let alice = device(1);
        let ops = ValidChatOps::new([
            (hash(1), message(alice, 1000, 0)),
            (hash(2), edit(alice, 2000, 1, hash(1))),
        ]);
        // A second edit of hash 1 would form a tree, not a chain.
        assert_eq!(
            EditCandidate {
                target: hash(1),
                editor: alice,
                timestamp: 2500,
                self_hash: None,
            }
            .validate(&ops),
            Err(EditError::AlreadyEdited)
        );
    }

    #[test]
    fn self_hash_excluded_from_already_edited_scan() {
        let alice = device(1);
        let ops = ValidChatOps::new([
            (hash(1), message(alice, 1000, 0)),
            (hash(2), edit(alice, 2000, 1, hash(1))),
        ]);
        // Validating the existing edit (hash 2) against its own target must not
        // see itself as a competing edit.
        assert_eq!(
            EditCandidate {
                target: hash(1),
                editor: alice,
                timestamp: 2000,
                self_hash: Some(hash(2)),
            }
            .validate(&ops),
            Ok(())
        );
    }

    #[test]
    fn competing_edits_earliest_seq_num_wins() {
        let alice = device(1);
        // hash(2) and hash(3) both edit the same original hash(1) — e.g. a UI
        // that bounced two edit clicks before the first was committed. The one
        // published earlier (lower seq_num) survives; the later one loses.
        let ops = ValidChatOps::new(HashMap::from([
            (hash(1), message(alice, 1000, 0)),
            (hash(2), edit(alice, 2000, 2, hash(1))),
            (hash(3), edit(alice, 2000, 1, hash(1))),
        ]));

        // hash(3) has the lower seq_num, so it wins
        assert_eq!(
            EditCandidate {
                target: hash(1),
                editor: alice,
                timestamp: 2000,
                self_hash: Some(hash(3)),
            }
            .validate(&ops),
            Ok(())
        );
        assert_eq!(
            EditCandidate {
                target: hash(1),
                editor: alice,
                timestamp: 2000,
                self_hash: Some(hash(2)),
            }
            .validate(&ops),
            Err(EditError::AlreadyEdited)
        );
    }

    #[test]
    fn competing_edits_same_seq_num_break_ties_by_hash() {
        let alice = device(1);
        // A forked log can reuse a seq_num; the lower op hash then wins so every
        // peer converges on the same survivor.
        let ops = ValidChatOps::new(HashMap::from([
            (hash(1), message(alice, 1000, 0)),
            (hash(2), edit(alice, 2000, 1, hash(1))),
            (hash(3), edit(alice, 2000, 1, hash(1))),
        ]));
        assert_eq!(
            EditCandidate {
                target: hash(1),
                editor: alice,
                timestamp: 2000,
                self_hash: Some(hash(2)),
            }
            .validate(&ops),
            Ok(())
        );
        assert_eq!(
            EditCandidate {
                target: hash(1),
                editor: alice,
                timestamp: 2000,
                self_hash: Some(hash(3)),
            }
            .validate(&ops),
            Err(EditError::AlreadyEdited)
        );
    }

    #[test]
    fn window_measured_from_root_message() {
        let alice = device(1);
        let ops = ValidChatOps::new(HashMap::from([
            (hash(1), message(alice, 1000, 0)),
            (hash(2), edit(alice, 1000 + EDIT_WINDOW_MICROS, 1, hash(1))),
        ]));
        // Editing the chained edit just within the window from the root is ok.
        assert_eq!(
            EditCandidate {
                target: hash(2),
                editor: alice,
                timestamp: 1000 + EDIT_WINDOW_MICROS,
                self_hash: None,
            }
            .validate(&ops),
            Ok(())
        );
        // Just past the window from the root is rejected, even though the direct
        // target (hash 2) is recent.
        assert_eq!(
            EditCandidate {
                target: hash(2),
                editor: alice,
                timestamp: 1000 + EDIT_WINDOW_MICROS + 1,
                self_hash: None,
            }
            .validate(&ops),
            Err(EditError::WindowExpired)
        );
    }

    #[test]
    fn window_expired_on_first_edit() {
        let alice = device(1);
        let ops = ValidChatOps::new(HashMap::from([(hash(1), message(alice, 1000, 0))]));
        assert_eq!(
            EditCandidate {
                target: hash(1),
                editor: alice,
                timestamp: 1000 + EDIT_WINDOW_MICROS + 1,
                self_hash: None,
            }
            .validate(&ops),
            Err(EditError::WindowExpired)
        );
    }
}
