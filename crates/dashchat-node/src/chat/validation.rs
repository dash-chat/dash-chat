mod delete;
mod edit;
mod reply;

#[cfg(test)]
pub(crate) mod test_common;

pub use delete::*;
pub use edit::*;
pub use reply::*;

use std::collections::BTreeSet;
use std::collections::HashMap;

use derive_more::{Deref, DerefMut, From};
use p2panda::Hash;

use crate::DeviceId;

/// A chat operation reduced to only the facts that edit/delete/reply validation cares about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatOpKind {
    /// An original `ChatPayload::Message`, carrying its reply target (if any)
    /// once that target has passed validation — see [`ValidChatOps::prune`].
    Message { reply: Option<Hash> },
    /// A `ChatPayload::EditMessage` pointing at the operation it edits.
    Edit(Hash),
    /// A `ChatPayload::DeleteMessage` carrying the full edit chain it deletes.
    Delete(BTreeSet<Hash>),
    /// Any other chat payload (reaction, group info, …) which are not
    /// valid targets for editing.
    Other,
}

/// A chat operation reduced to only the fields edit/delete validation needs.
///
/// By writing validation in terms of `ChatOp`s instead of full `ChatPayload`s,
/// we keep the scope of validation bounded to only the relevant facts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatOp {
    pub author: DeviceId,
    /// Microseconds since the UNIX epoch (the operation header timestamp).
    pub timestamp: u64,
    /// Position in the author's append-only log; monotonic in publish order.
    pub seq_num: u64,
    pub kind: ChatOpKind,
}

/// The set of all chat operations in a topic that pass edit/delete validation,
/// keyed by operation hash. Callers obtain a pruned, cheat-proof view via
/// [`Node::valid_chat_ops`](crate::Node) and validate candidate edits/deletes
/// against it with [`EditCandidate::validate`] / [`DeleteCandidate::validate`].
#[derive(Debug, Clone, Default, Deref, DerefMut, From)]
pub struct ValidChatOps(HashMap<Hash, ChatOp>);

impl ValidChatOps {
    pub(crate) fn new(ops: impl IntoIterator<Item = (Hash, ChatOp)>) -> Self {
        Self(ops.into_iter().collect())
    }

    pub(crate) fn contains(&self, hash: &Hash) -> bool {
        self.0.contains_key(hash)
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
                    ChatOpKind::Edit(edit_hash) => EditCandidate {
                        target: edit_hash,
                        editor: author,
                        timestamp,
                        self_hash: Some(hash),
                    }
                    .validate(self)
                    .is_ok(),
                    ChatOpKind::Delete(hashes) => DeleteCandidate {
                        hashes,
                        deleter: author,
                        delete_timestamp: timestamp,
                        self_hash: Some(hash),
                    }
                    .validate(self)
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

        self.prune_invalid_replies();
    }

    // Clear reply annotations that don't pass validation, so callers can
    // trust a `Some` reply target on a pruned `Message` without re-running
    // `ReplyCandidate::validate` themselves. Unlike edits/deletes, an invalid
    // reply never removes the op carrying it — the message stays, only the
    // quote is dropped — and clearing a reply can't invalidate anything else,
    // so a single pass (after edit/delete pruning above) is enough.
    fn prune_invalid_replies(&mut self) {
        let candidates: Vec<(Hash, Hash, u64)> = self
            .0
            .iter()
            .filter_map(|(hash, op)| match op.kind {
                ChatOpKind::Message {
                    reply: Some(target),
                } => Some((*hash, target, op.timestamp)),
                _ => None,
            })
            .collect();
        for (hash, target, timestamp) in candidates {
            let valid = ReplyCandidate {
                target,
                timestamp,
                self_hash: Some(hash),
            }
            .validate(self)
            .is_ok();
            if !valid {
                if let Some(op) = self.0.get_mut(&hash) {
                    op.kind = ChatOpKind::Message { reply: None };
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_common::*;
    use super::*;

    #[test]
    fn prune_clears_a_reply_targeting_an_unknown_message() {
        let alice = device(1);
        let bobbi = device(2);
        let mut ops = ValidChatOps::new([
            (hash(1), message(alice, 1000, 0)),
            (
                hash(2),
                ChatOp {
                    author: bobbi,
                    timestamp: 2000,
                    seq_num: 0,
                    kind: ChatOpKind::Message {
                        reply: Some(hash(9)),
                    },
                },
            ),
        ]);

        ops.prune();

        assert_eq!(
            ops.get(&hash(2)).unwrap().kind,
            ChatOpKind::Message { reply: None }
        );
    }

    #[test]
    fn prune_keeps_a_reply_targeting_a_known_message() {
        let alice = device(1);
        let bobbi = device(2);
        let mut ops = ValidChatOps::new([
            (hash(1), message(alice, 1000, 0)),
            (
                hash(2),
                ChatOp {
                    author: bobbi,
                    timestamp: 2000,
                    seq_num: 0,
                    kind: ChatOpKind::Message {
                        reply: Some(hash(1)),
                    },
                },
            ),
        ]);

        ops.prune();

        assert_eq!(
            ops.get(&hash(2)).unwrap().kind,
            ChatOpKind::Message {
                reply: Some(hash(1))
            }
        );
    }

    #[test]
    fn prune_clears_a_reply_to_a_message_deleted_in_the_same_pass() {
        let alice = device(1);
        let bobbi = device(2);
        let mut ops = ValidChatOps::new([
            (hash(1), message(alice, 1000, 0)),
            (hash(2), delete(alice, 2000, 1, maplit::btreeset![hash(1)])),
            (
                hash(3),
                ChatOp {
                    author: bobbi,
                    timestamp: 3000,
                    seq_num: 0,
                    kind: ChatOpKind::Message {
                        reply: Some(hash(1)),
                    },
                },
            ),
        ]);

        ops.prune();

        assert_eq!(
            ops.get(&hash(3)).unwrap().kind,
            ChatOpKind::Message { reply: None }
        );
    }
}
