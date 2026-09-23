use super::*;

pub fn device(n: u8) -> DeviceId {
    DeviceId::from(p2panda::SigningKey::from_bytes(&[n; 32]).verifying_key())
}

pub fn hash(n: u8) -> Hash {
    Hash::from_bytes([n; 32])
}

pub fn message(author: DeviceId, timestamp: u64, seq_num: u64) -> ChatOp {
    ChatOp {
        author,
        timestamp,
        seq_num,
        kind: ChatOpKind::Message { reply: None },
    }
}

pub fn edit(author: DeviceId, timestamp: u64, seq_num: u64, target: Hash) -> ChatOp {
    ChatOp {
        author,
        timestamp,
        seq_num,
        kind: ChatOpKind::Edit(target),
    }
}

pub fn delete(author: DeviceId, timestamp: u64, seq_num: u64, hashes: BTreeSet<Hash>) -> ChatOp {
    ChatOp {
        author,
        timestamp,
        seq_num,
        kind: ChatOpKind::Delete(hashes),
    }
}

pub fn other(author: DeviceId, timestamp: u64, seq_num: u64) -> ChatOp {
    ChatOp {
        author,
        timestamp,
        seq_num,
        kind: ChatOpKind::Other,
    }
}
