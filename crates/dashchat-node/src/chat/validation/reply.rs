use p2panda::Hash;
use serde::Serialize;
use thiserror::Error;

use super::{ChatOpKind, ValidChatOps};

/// Why a reply annotation on a message is considered invalid.
///
/// The same validation rules are enforced on the author's side (as a hard error
/// before publishing) and on the receiving side — except that the receiver only
/// ignores the reply annotation, never the message carrying it, and does not
/// enforce [`ReplyError::NotLatestEdit`] (the replier may honestly not have
/// known of a later edit).
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize)]
#[serde(tag = "kind", content = "message")]
pub enum ReplyError {
    #[error("the message being replied to could not be found in this chat")]
    TargetNotFound,

    #[error("only messages and their edits can be replied to")]
    TargetNotRepliable,

    #[error("the message being replied to has been deleted")]
    TargetDeleted,

    #[error("a reply must be later than the message it replies to")]
    TimestampNotLater,

    #[error("a reply must target the most recent edit of a message")]
    NotLatestEdit,
}

/// A received reply that passed validation, exposed for tests.
#[cfg(any(test, feature = "testing"))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidReply {
    /// Hash of the message operation carrying the reply.
    pub op_hash: Hash,
    /// Hash of the operation it replies to.
    pub target: Hash,
    /// The reply's own text content.
    pub text: String,
}

/// A candidate reply to be validated against its target message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplyCandidate {
    /// The operation the reply points at.
    pub target: Hash,
    pub timestamp: u64,
    /// `None` when an author is about to publish a reply (all rules are
    /// enforced as a hard error, including targeting the latest known edit).
    /// `Some` when a receiver is validating a reply published by someone else
    /// (the latest-edit rule cannot be enforced there: the replier may not
    /// have known of a later edit).
    pub self_hash: Option<Hash>,
}

impl ReplyCandidate {
    /// Validate that this reply may point at its target message,
    /// given the set of existing valid chat ops.
    pub fn validate(&self, valid_ops: &ValidChatOps) -> Result<(), ReplyError> {
        self.check_target_repliable(valid_ops)?;
        self.check_target_not_deleted(valid_ops)?;
        self.check_timestamp_later(valid_ops)?;
        if self.self_hash.is_none() {
            self.check_target_is_latest_edit(valid_ops)?;
        }
        Ok(())
    }

    fn check_target_repliable(&self, valid_ops: &ValidChatOps) -> Result<(), ReplyError> {
        let target = valid_ops
            .get(&self.target)
            .ok_or(ReplyError::TargetNotFound)?;
        match target.kind {
            ChatOpKind::Message | ChatOpKind::Edit(_) => Ok(()),
            ChatOpKind::Delete(_) | ChatOpKind::Other => Err(ReplyError::TargetNotRepliable),
        }
    }

    // A processed delete tombstones its targets, so they usually already fail
    // the lookup above; this catches a delete that covers the target but has
    // not been applied locally yet.
    fn check_target_not_deleted(&self, valid_ops: &ValidChatOps) -> Result<(), ReplyError> {
        let deleted = valid_ops.values().any(
            |op| matches!(&op.kind, ChatOpKind::Delete(covered) if covered.contains(&self.target)),
        );
        if deleted {
            return Err(ReplyError::TargetDeleted);
        }
        Ok(())
    }

    fn check_timestamp_later(&self, valid_ops: &ValidChatOps) -> Result<(), ReplyError> {
        let target = valid_ops
            .get(&self.target)
            .ok_or(ReplyError::TargetNotFound)?;
        if self.timestamp <= target.timestamp {
            return Err(ReplyError::TimestampNotLater);
        }
        Ok(())
    }

    fn check_target_is_latest_edit(&self, valid_ops: &ValidChatOps) -> Result<(), ReplyError> {
        let target_is_edited = valid_ops
            .values()
            .any(|op| matches!(&op.kind, ChatOpKind::Edit(t) if t == &self.target));
        if target_is_edited {
            return Err(ReplyError::NotLatestEdit);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use maplit::btreeset;

    use super::super::ChatOp;
    use super::*;
    use crate::DeviceId;

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
    fn valid_reply_to_message() {
        let alice = device(1);
        let ops = ValidChatOps::new([(hash(1), message(alice, 1000, 0))]);
        assert_eq!(
            ReplyCandidate {
                target: hash(1),
                timestamp: 2000,
                self_hash: None,
            }
            .validate(&ops),
            Ok(())
        );
    }

    #[test]
    fn valid_reply_to_latest_edit() {
        let alice = device(1);
        let ops = ValidChatOps::new([
            (hash(1), message(alice, 1000, 0)),
            (hash(2), edit(alice, 2000, 1, hash(1))),
        ]);
        assert_eq!(
            ReplyCandidate {
                target: hash(2),
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
            ReplyCandidate {
                target: hash(9),
                timestamp: 2000,
                self_hash: None,
            }
            .validate(&ops),
            Err(ReplyError::TargetNotFound)
        );
    }

    #[test]
    fn reply_to_non_message_is_rejected() {
        let alice = device(1);
        let ops = ValidChatOps::new([(hash(1), other(alice, 1000, 0))]);
        assert_eq!(
            ReplyCandidate {
                target: hash(1),
                timestamp: 2000,
                self_hash: None,
            }
            .validate(&ops),
            Err(ReplyError::TargetNotRepliable)
        );
    }

    #[test]
    fn reply_to_a_delete_is_rejected() {
        let alice = device(1);
        let ops = ValidChatOps::new([
            (hash(1), message(alice, 1000, 0)),
            (hash(2), delete(alice, 2000, 1, btreeset![hash(1)])),
        ]);
        assert_eq!(
            ReplyCandidate {
                target: hash(2),
                timestamp: 3000,
                self_hash: None,
            }
            .validate(&ops),
            Err(ReplyError::TargetNotRepliable)
        );
    }

    #[test]
    fn reply_to_a_deleted_message_is_rejected() {
        let alice = device(1);
        // The delete has not been applied yet, so the target still resolves.
        let ops = ValidChatOps::new([
            (hash(1), message(alice, 1000, 0)),
            (hash(2), delete(alice, 2000, 1, btreeset![hash(1)])),
        ]);
        assert_eq!(
            ReplyCandidate {
                target: hash(1),
                timestamp: 3000,
                self_hash: None,
            }
            .validate(&ops),
            Err(ReplyError::TargetDeleted)
        );
    }

    #[test]
    fn reply_must_be_later_than_target() {
        let alice = device(1);
        let ops = ValidChatOps::new([(hash(1), message(alice, 1000, 0))]);
        assert_eq!(
            ReplyCandidate {
                target: hash(1),
                timestamp: 1000,
                self_hash: None,
            }
            .validate(&ops),
            Err(ReplyError::TimestampNotLater)
        );
        assert_eq!(
            ReplyCandidate {
                target: hash(1),
                timestamp: 999,
                self_hash: None,
            }
            .validate(&ops),
            Err(ReplyError::TimestampNotLater)
        );
        assert_eq!(
            ReplyCandidate {
                target: hash(1),
                timestamp: 1001,
                self_hash: None,
            }
            .validate(&ops),
            Ok(())
        );
    }

    #[test]
    fn author_cannot_reply_to_a_superseded_edit() {
        let alice = device(1);
        let ops = ValidChatOps::new([
            (hash(1), message(alice, 1000, 0)),
            (hash(2), edit(alice, 2000, 1, hash(1))),
        ]);
        // Publishing a reply to the original while knowing of the edit is
        // dishonest and rejected locally.
        assert_eq!(
            ReplyCandidate {
                target: hash(1),
                timestamp: 3000,
                self_hash: None,
            }
            .validate(&ops),
            Err(ReplyError::NotLatestEdit)
        );
    }

    #[test]
    fn receiver_accepts_reply_to_a_superseded_edit() {
        let alice = device(1);
        let ops = ValidChatOps::new([
            (hash(1), message(alice, 1000, 0)),
            (hash(2), edit(alice, 2000, 1, hash(1))),
        ]);
        // The replier may not have known of the edit yet; a receiver cannot
        // enforce the latest-edit rule.
        assert_eq!(
            ReplyCandidate {
                target: hash(1),
                timestamp: 3000,
                self_hash: Some(hash(3)),
            }
            .validate(&ops),
            Ok(())
        );
    }
}
