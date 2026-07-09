use std::collections::{BTreeSet, HashMap};

use derive_more::derive::{Deref, DerefMut, From};
use p2panda::Hash;
use serde::Serialize;
use thiserror::Error;

use crate::{DeleteCandidate, DeviceId};

/// The window during which a message may be edited, measured from the original
/// message timestamp. p2panda header timestamps are microseconds since the UNIX
/// epoch, so this is 24 hours expressed in microseconds.
pub const EDIT_WINDOW_MICROS: u64 = 24 * 60 * 60 * 1_000_000;

/// Why an edit operation is considered invalid
/// (not allowed to be applied to its target).
///
/// The same validation rules are enforced on the author's side (as a hard error before
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

/// A chat operation reduced to only the facts that edit/delete validation cares about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatOpKind {
    /// An original `ChatPayload::Message`.
    Message,
    /// A `ChatPayload::EditMessage` pointing at the operation it edits.
    Edit(Hash),
    /// A `ChatPayload::DeleteMessage` carrying the full edit chain it deletes.
    Delete(BTreeSet<Hash>),
    /// Any other chat payload (reaction, group info, …) which are not
    /// valid targets for editing.
    Other,
}

/// A chat operation reduced to only the fields edit validation needs.
///
/// By writing edit validation in terms of `ChatOp`s instead of full `ChatPayload`s,
/// we keep the scope of edit validation bounded to only the relevant facts.
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

/// The set of all chat operations in a topic that pass edit validation, keyed
/// by operation hash. Callers obtain a pruned, cheat-proof view via
/// [`Node::valid_chat_ops`](crate::Node) and validate candidate edits against
/// it with [`ValidChatOps::validate`].
#[derive(Debug, Clone, Default, Deref, DerefMut, From)]
pub struct ValidChatOps(HashMap<Hash, ChatOp>);

impl ValidChatOps {
    pub(crate) fn new(ops: impl IntoIterator<Item = (Hash, ChatOp)>) -> Self {
        Self(ops.into_iter().collect())
    }

    pub(crate) fn contains(&self, hash: &Hash) -> bool {
        self.0.contains_key(hash)
    }

    /// Validate that `edit` may be applied to its target message,
    /// given the set of existing valid chat ops.
    pub fn validate_edit(&self, edit: &EditCandidate) -> Result<(), EditError> {
        let target = self.get(&edit.target).ok_or(EditError::TargetNotFound)?;
        Self::check_target_editable(target)?;
        Self::check_editor_is_author(target, edit)?;
        self.check_not_conflicting(edit)?;
        self.check_within_edit_window(edit)?;
        Ok(())
    }

    // Strip edit and delete ops that don't pass validation so callers
    // always work with a consistent, cheat-proof view. An invalid op (wrong
    // author, expired window, or broken chain) must not poison the scan for
    // legitimate ones. Removals cascade — a chained edit whose target gets
    // stripped is itself invalid — so iterate to a fixpoint. Validation
    // depends only on the op set, not arrival order, so every peer
    // converges on the same reduced view. (A delete that has already been
    // applied gets stripped too — its targets' bodies are gone — which is
    // harmless: the dropped bodies themselves block any further edit or
    // delete of those operations.)
    pub(crate) fn prune(&mut self) {
        loop {
            let candidates: Vec<Hash> = self
                .0
                .iter()
                .filter_map(|(hash, op)| {
                    matches!(op.kind, ChatOpKind::Edit(_) | ChatOpKind::Delete(_)).then_some(*hash)
                })
                .collect();
            let mut removed_any = false;
            for hash in candidates {
                let Some(op) = self.get(&hash) else {
                    continue;
                };
                let (kind, author, timestamp) = (op.kind.clone(), op.author, op.timestamp);
                let valid = match kind {
                    ChatOpKind::Edit(edit_hash) => self
                        .validate_edit(&EditCandidate {
                            target: edit_hash,
                            editor: author,
                            timestamp,
                            self_hash: Some(hash),
                        })
                        .is_ok(),
                    ChatOpKind::Delete(hashes) => self
                        .validate_delete(&DeleteCandidate {
                            hashes,
                            deleter: author,
                            delete_timestamp: timestamp,
                            self_hash: Some(hash),
                        })
                        .is_ok(),
                    _ => unreachable!(),
                };
                if !valid {
                    self.0.remove(&hash);
                    removed_any = true;
                }
            }
            if !removed_any {
                break;
            }
        }
    }

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

    fn check_target_editable(target: &ChatOp) -> Result<(), EditError> {
        match target.kind {
            ChatOpKind::Message | ChatOpKind::Edit(_) => Ok(()),
            ChatOpKind::Delete(_) | ChatOpKind::Other => return Err(EditError::TargetNotEditable),
        }
    }

    // TODO: this is only a same-device check.
    // If we want editing across devices, we need to check against AgentId, which is more complicated.
    fn check_editor_is_author(target: &ChatOp, edit: &EditCandidate) -> Result<(), EditError> {
        if edit.editor != target.author {
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
    fn check_not_conflicting(&self, edit: &EditCandidate) -> Result<(), EditError> {
        // The order key for this candidate, to be compared against
        // the order key of any potentially conflicting edits.
        let self_order_key = edit
            .self_hash
            .and_then(|h| self.get(&h).map(|op| (op.seq_num, h)));
        let invalid = self.iter().any(|(hash, op)| {
            let conflicting = matches!(&op.kind, ChatOpKind::Edit(t) if t == &edit.target);
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

    fn check_within_edit_window(&self, edit: &EditCandidate) -> Result<(), EditError> {
        let root_timestamp = self
            .root_message_timestamp_for_edit_chain(&edit.target)
            .ok_or(EditError::TargetNotFound)?;
        if edit.timestamp.saturating_sub(root_timestamp) > EDIT_WINDOW_MICROS {
            return Err(EditError::WindowExpired);
        }
        Ok(())
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
        let ops = ValidChatOps::new([(hash(1), message(alice, 1000, 0))]);
        assert_eq!(
            ops.validate_edit(&EditCandidate {
                target: hash(1),
                editor: alice,
                timestamp: 2000,
                self_hash: None,
            }),
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
            ops.validate_edit(&EditCandidate {
                target: hash(2),
                editor: alice,
                timestamp: 3000,
                self_hash: None,
            }),
            Ok(())
        );
    }

    #[test]
    fn target_not_found() {
        let alice = device(1);
        let ops = ValidChatOps::new([(hash(1), message(alice, 1000, 0))]);
        assert_eq!(
            ops.validate_edit(&EditCandidate {
                target: hash(9),
                editor: alice,
                timestamp: 2000,
                self_hash: None,
            }),
            Err(EditError::TargetNotFound)
        );
    }

    #[test]
    fn target_not_editable() {
        let alice = device(1);
        // hash 1 is a reaction / group-info / etc.
        let ops = ValidChatOps::new([(hash(1), other(alice, 1000, 0))]);
        assert_eq!(
            ops.validate_edit(&EditCandidate {
                target: hash(1),
                editor: alice,
                timestamp: 2000,
                self_hash: None,
            }),
            Err(EditError::TargetNotEditable)
        );
    }

    #[test]
    fn not_author() {
        let alice = device(1);
        let bobbi = device(2);
        let ops = ValidChatOps::new([(hash(1), message(alice, 1000, 0))]);
        assert_eq!(
            ops.validate_edit(&EditCandidate {
                target: hash(1),
                editor: bobbi,
                timestamp: 2000,
                self_hash: None,
            }),
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
            ops.validate_edit(&EditCandidate {
                target: hash(1),
                editor: alice,
                timestamp: 2500,
                self_hash: None,
            }),
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
            ops.validate_edit(&EditCandidate {
                target: hash(1),
                editor: alice,
                timestamp: 2000,
                self_hash: Some(hash(2)),
            }),
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
            ops.validate_edit(&EditCandidate {
                target: hash(1),
                editor: alice,
                timestamp: 2000,
                self_hash: Some(hash(3)),
            }),
            Ok(())
        );
        assert_eq!(
            ops.validate_edit(&EditCandidate {
                target: hash(1),
                editor: alice,
                timestamp: 2000,
                self_hash: Some(hash(2)),
            }),
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
            ops.validate_edit(&EditCandidate {
                target: hash(1),
                editor: alice,
                timestamp: 2000,
                self_hash: Some(hash(2)),
            }),
            Ok(())
        );
        assert_eq!(
            ops.validate_edit(&EditCandidate {
                target: hash(1),
                editor: alice,
                timestamp: 2000,
                self_hash: Some(hash(3)),
            }),
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
            ops.validate_edit(&EditCandidate {
                target: hash(2),
                editor: alice,
                timestamp: 1000 + EDIT_WINDOW_MICROS,
                self_hash: None,
            }),
            Ok(())
        );
        // Just past the window from the root is rejected, even though the direct
        // target (hash 2) is recent.
        assert_eq!(
            ops.validate_edit(&EditCandidate {
                target: hash(2),
                editor: alice,
                timestamp: 1000 + EDIT_WINDOW_MICROS + 1,
                self_hash: None,
            }),
            Err(EditError::WindowExpired)
        );
    }

    #[test]
    fn window_expired_on_first_edit() {
        let alice = device(1);
        let ops = ValidChatOps::new(HashMap::from([(hash(1), message(alice, 1000, 0))]));
        assert_eq!(
            ops.validate_edit(&EditCandidate {
                target: hash(1),
                editor: alice,
                timestamp: 1000 + EDIT_WINDOW_MICROS + 1,
                self_hash: None,
            }),
            Err(EditError::WindowExpired)
        );
    }
}
