use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::{Opaq, OpaqHash};

pub type TopicId = String;
pub type Author = String;
pub type SequenceNumber = u64;

#[derive(Serialize, Deserialize)]
pub struct GetDollopsRequest {
    pub topics: BTreeMap<TopicId, BTreeMap<Author, SequenceNumber>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetDollopsForTopicResponse {
    // The dollops that the client does not have
    pub dollops: BTreeMap<Author, BTreeMap<SequenceNumber, Opaq>>,
    // The dollops that the server is missing from the client's request
    pub missing: BTreeMap<Author, Vec<SequenceNumber>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetDollopsResponse {
    pub dollops_by_topic: BTreeMap<TopicId, GetDollopsForTopicResponse>,
}

#[derive(Serialize, Deserialize)]
pub struct StoreDollopsRequest {
    pub dollops: BTreeMap<TopicId, BTreeMap<Author, BTreeMap<SequenceNumber, Opaq>>>,
}

#[derive(Serialize, Deserialize)]
pub struct GetBlobsRequest {
    pub blob_hashes: Vec<OpaqHash>,
}

#[derive(Serialize, Deserialize)]
pub struct GetBlobsResponse {
    pub blobs: Vec<Opaq>,
}

#[derive(Serialize, Deserialize)]
pub struct StoreBlobsRequest {
    pub blobs: Vec<Opaq>,
}
