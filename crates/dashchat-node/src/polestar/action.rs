use p2panda::Hash;
use serde::{Deserialize, Serialize};

use crate::topic::TopicId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Action {
    Space(SpaceAction),
    AuthorOp { topic: TopicId, hash: Hash },
    ProcessOp,
    BufferOp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SpaceAction {}
