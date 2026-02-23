use redb::{Key, TableDefinition, TypeName, Value};
use std::cmp::Ordering;
use std::fmt;
use uuid::Uuid;

use crate::watermarks_table::WatermarksKey;

/// Error type for blobs key operations
#[derive(Debug, thiserror::Error)]
pub enum BlobsKeyError {
    #[error("Topic ID contains invalid character (colon or null): {0}")]
    InvalidTopicId(String),
    #[error("Author contains invalid character (colon or null): {0}")]
    InvalidAuthor(String),
    #[error("Failed to parse key: {0}")]
    ParseError(String),
}

/// Key for BLOBS_TABLE with binary format for efficient comparison.
///
/// Binary format: `topic_id + 0x00 + author + 0x00 + seq_be8 + uuid_16`
/// - topic_id: UTF-8 bytes (no null bytes allowed)
/// - 0x00: null byte delimiter
/// - author: UTF-8 bytes (no null bytes allowed)
/// - 0x00: null byte delimiter
/// - seq_be8: sequence number as 8 bytes big-endian
/// - uuid_16: UUID as 16 raw bytes
///
/// This format enables direct byte comparison that matches struct field ordering.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct BlobsKey {
    // NOTE: order of these fields matters!
    pub topic_id: String,
    pub author: String,
    pub sequence_number: u64,
    pub uuid: Uuid,
}

impl BlobsKey {
    /// Creates a new BlobsKey with validation
    pub fn new(
        topic_id: String,
        author: String,
        sequence_number: u64,
        uuid: Uuid,
    ) -> Result<Self, BlobsKeyError> {
        if topic_id.contains(':') || topic_id.contains('\0') {
            return Err(BlobsKeyError::InvalidTopicId(topic_id));
        }
        if author.contains(':') || author.contains('\0') {
            return Err(BlobsKeyError::InvalidAuthor(author));
        }
        Ok(Self {
            topic_id,
            author,
            sequence_number,
            uuid,
        })
    }

    /// Creates a new BlobsKey with a fresh UUID v7 timestamp
    pub fn new_now(
        topic_id: String,
        author: String,
        sequence_number: u64,
    ) -> Result<Self, BlobsKeyError> {
        Self::new(topic_id, author, sequence_number, Uuid::now_v7())
    }

    /// Parses a BlobsKey from its string representation
    pub fn parse(s: &str) -> Result<Self, BlobsKeyError> {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() != 4 {
            return Err(BlobsKeyError::ParseError(format!(
                "Expected 4 parts, got {}",
                parts.len()
            )));
        }

        let topic_id = parts[0].to_string();
        let author = parts[1].to_string();
        let sequence_number = parts[2].parse::<u64>().map_err(|e| {
            BlobsKeyError::ParseError(format!("Invalid sequence number '{}': {}", parts[2], e))
        })?;
        let uuid = Uuid::parse_str(parts[3]).map_err(|e| {
            BlobsKeyError::ParseError(format!("Invalid UUID '{}': {}", parts[3], e))
        })?;

        Ok(Self {
            topic_id,
            author,
            sequence_number,
            uuid,
        })
    }

    /// Extracts the WatermarksKey (topic_id:author) from this BlobsKey
    pub fn watermarks_key(&self) -> WatermarksKey {
        // Safe to unwrap because BlobsKey already validated no colons
        WatermarksKey::new(self.topic_id.clone(), self.author.clone()).unwrap()
    }
}

impl fmt::Display for BlobsKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}:{:020}:{}",
            self.topic_id, self.author, self.sequence_number, self.uuid
        )
    }
}

impl Value for BlobsKey {
    type SelfType<'a> = BlobsKey;
    type AsBytes<'a> = Vec<u8>;

    fn fixed_width() -> Option<usize> {
        None
    }

    fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
    where
        Self: 'a,
    {
        // Find first null byte (end of topic_id)
        let first_null = data
            .iter()
            .position(|&b| b == 0)
            .expect("Missing first null delimiter in BlobsKey");
        let topic_id = std::str::from_utf8(&data[..first_null])
            .expect("Invalid UTF-8 in topic_id")
            .to_string();

        // Find second null byte (end of author)
        let rest = &data[first_null + 1..];
        let second_null = rest
            .iter()
            .position(|&b| b == 0)
            .expect("Missing second null delimiter in BlobsKey");
        let author = std::str::from_utf8(&rest[..second_null])
            .expect("Invalid UTF-8 in author")
            .to_string();

        // Read sequence number (8 bytes big-endian)
        let seq_start = first_null + 1 + second_null + 1;
        let sequence_number = u64::from_be_bytes(
            data[seq_start..seq_start + 8]
                .try_into()
                .expect("Invalid sequence number bytes"),
        );

        // Read UUID (16 bytes)
        let uuid_start = seq_start + 8;
        let uuid = Uuid::from_bytes(
            data[uuid_start..uuid_start + 16]
                .try_into()
                .expect("Invalid UUID bytes"),
        );

        BlobsKey {
            topic_id,
            author,
            sequence_number,
            uuid,
        }
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a>
    where
        Self: 'b,
    {
        let mut bytes =
            Vec::with_capacity(value.topic_id.len() + 1 + value.author.len() + 1 + 8 + 16);
        bytes.extend_from_slice(value.topic_id.as_bytes());
        bytes.push(0);
        bytes.extend_from_slice(value.author.as_bytes());
        bytes.push(0);
        bytes.extend_from_slice(&value.sequence_number.to_be_bytes());
        bytes.extend_from_slice(value.uuid.as_bytes());
        bytes
    }

    fn type_name() -> TypeName {
        TypeName::new("mailbox_server::BlobsKey")
    }
}

impl Key for BlobsKey {
    fn compare(data1: &[u8], data2: &[u8]) -> Ordering {
        // Direct byte comparison preserves ordering because:
        // - Null byte (0x00) delimiters are smaller than any valid UTF-8 byte
        // - Strings compare lexicographically by UTF-8 bytes
        // - Sequence number is 8 bytes big-endian (preserves numeric ordering)
        // - UUID is 16 raw bytes (preserves Uuid::cmp ordering)
        data1.cmp(data2)
    }
}

/// Partial key for prefix-based range queries on BLOBS_TABLE
#[derive(Debug, Clone)]
pub enum BlobsKeyPrefix {
    /// Match all keys for a topic: "topic_id:"
    Topic(String),
    /// Match all keys for a topic:author: "topic_id:author:"
    TopicAuthor(String, String),
    /// Match all keys for a topic:author:seq: "topic_id:author:seq:"
    TopicAuthorSeq(String, String, u64),
}

impl BlobsKeyPrefix {
    /// Returns a BlobsKey for the lower bound of a range query.
    /// Uses minimal values (empty author, seq 0, nil UUID) for unspecified parts.
    pub fn range_start(&self) -> BlobsKey {
        match self {
            BlobsKeyPrefix::Topic(topic) => BlobsKey {
                topic_id: topic.clone(),
                author: String::new(),
                sequence_number: 0,
                uuid: Uuid::nil(),
            },
            BlobsKeyPrefix::TopicAuthor(topic, author) => BlobsKey {
                topic_id: topic.clone(),
                author: author.clone(),
                sequence_number: 0,
                uuid: Uuid::nil(),
            },
            BlobsKeyPrefix::TopicAuthorSeq(topic, author, seq) => BlobsKey {
                topic_id: topic.clone(),
                author: author.clone(),
                sequence_number: *seq,
                uuid: Uuid::nil(),
            },
        }
    }

    /// Returns a BlobsKey for the upper bound of a range query (exclusive).
    /// Uses maximal values for unspecified parts.
    pub fn range_end(&self) -> BlobsKey {
        match self {
            BlobsKeyPrefix::Topic(topic) => BlobsKey {
                topic_id: topic.clone(),
                // U+FFFF is the highest Unicode code point, sorts after all valid authors
                author: String::from("\u{FFFF}"),
                sequence_number: u64::MAX,
                uuid: Uuid::max(),
            },
            BlobsKeyPrefix::TopicAuthor(topic, author) => BlobsKey {
                topic_id: topic.clone(),
                author: author.clone(),
                sequence_number: u64::MAX,
                uuid: Uuid::max(),
            },
            BlobsKeyPrefix::TopicAuthorSeq(topic, author, seq) => BlobsKey {
                topic_id: topic.clone(),
                author: author.clone(),
                sequence_number: *seq,
                uuid: Uuid::max(),
            },
        }
    }
}

// Database key format: topic_id + 0x00 + author + 0x00 + seq_be8 + uuid_16
// The UUID v7 suffix is used for cleanup based on message age
// Binary format enables direct byte comparison for efficient database operations
pub const BLOBS_TABLE: TableDefinition<BlobsKey, &[u8]> = TableDefinition::new("blobs");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blobs_key_roundtrip() {
        let uuid = Uuid::now_v7();
        let key = BlobsKey::new("topic1".into(), "author1".into(), 42, uuid).unwrap();
        let serialized = key.to_string();
        let parsed = BlobsKey::parse(&serialized).unwrap();
        assert_eq!(key, parsed);
    }

    #[test]
    fn test_blobs_key_zero_padding() {
        let uuid = Uuid::now_v7();
        let key = BlobsKey::new("topic".into(), "author".into(), 5, uuid).unwrap();
        let serialized = key.to_string();
        assert!(serialized.contains(":00000000000000000005:"));
    }

    #[test]
    fn test_blobs_key_ordering() {
        let uuid = Uuid::now_v7();
        let key9 = BlobsKey::new("topic".into(), "author".into(), 9, uuid).unwrap();
        let key10 = BlobsKey::new("topic".into(), "author".into(), 10, uuid).unwrap();

        // With zero-padding, 9 should sort before 10
        assert!(key9.to_string() < key10.to_string());
    }

    #[test]
    fn test_blobs_key_rejects_colon_in_topic() {
        let uuid = Uuid::now_v7();
        let result = BlobsKey::new("topic:bad".into(), "author".into(), 0, uuid);
        assert!(matches!(result, Err(BlobsKeyError::InvalidTopicId(_))));
    }

    #[test]
    fn test_blobs_key_rejects_colon_in_author() {
        let uuid = Uuid::now_v7();
        let result = BlobsKey::new("topic".into(), "author:bad".into(), 0, uuid);
        assert!(matches!(result, Err(BlobsKeyError::InvalidAuthor(_))));
    }

    #[test]
    fn test_blobs_key_rejects_null_in_topic() {
        let uuid = Uuid::now_v7();
        let result = BlobsKey::new("topic\0bad".into(), "author".into(), 0, uuid);
        assert!(matches!(result, Err(BlobsKeyError::InvalidTopicId(_))));
    }

    #[test]
    fn test_blobs_key_rejects_null_in_author() {
        let uuid = Uuid::now_v7();
        let result = BlobsKey::new("topic".into(), "author\0bad".into(), 0, uuid);
        assert!(matches!(result, Err(BlobsKeyError::InvalidAuthor(_))));
    }

    #[test]
    fn test_blobs_key_binary_roundtrip() {
        let uuid = Uuid::now_v7();
        let key = BlobsKey::new("topic1".into(), "author1".into(), 42, uuid).unwrap();
        let bytes = BlobsKey::as_bytes(&key);
        let parsed = BlobsKey::from_bytes(&bytes);
        assert_eq!(key, parsed);
    }

    #[test]
    fn test_watermarks_key_from_blobs_key() {
        let uuid = Uuid::now_v7();
        let blobs_key = BlobsKey::new("topic".into(), "author".into(), 42, uuid).unwrap();
        let watermarks_key = blobs_key.watermarks_key();
        assert_eq!(watermarks_key.topic_id, "topic");
        assert_eq!(watermarks_key.author, "author");
    }

    #[test]
    fn test_prefix_range_topic() {
        let prefix = BlobsKeyPrefix::Topic(
            "d8883c1402ed3c078953620a5bf2afc8fafca9601186e7133ca6b1bf72c35cfb".into(),
        );
        let start = prefix.range_start();
        let end = prefix.range_end();

        assert!(start < end);
        assert_eq!(
            BlobsKey::compare(&BlobsKey::as_bytes(&start), &BlobsKey::as_bytes(&end)),
            Ordering::Less
        );

        let key = BlobsKey::new(
            "d8883c1402ed3c078953620a5bf2afc8fafca9601186e7133ca6b1bf72c35cfb".into(),
            "3cb6797ce981200974303722ca17cbd2691593f2b05fbe5b6152f0b813127a7e".into(),
            0,
            Uuid::parse_str("019be63b-efd7-7200-b7f5-d3d3945e989a").unwrap(),
        )
        .unwrap();

        assert!(start < key);
        assert!(key < end);

        // Making sure that database comparison works correctly
        assert_eq!(
            BlobsKey::compare(&BlobsKey::as_bytes(&start), &BlobsKey::as_bytes(&key)),
            Ordering::Less
        );
        assert_eq!(
            BlobsKey::compare(&BlobsKey::as_bytes(&key), &BlobsKey::as_bytes(&end)),
            Ordering::Less
        );

        assert_eq!(
            start.topic_id,
            "d8883c1402ed3c078953620a5bf2afc8fafca9601186e7133ca6b1bf72c35cfb"
        );
        assert_eq!(start.author, "");
        assert_eq!(start.sequence_number, 0);
        assert_eq!(start.uuid, Uuid::nil());

        assert_eq!(
            end.topic_id,
            "d8883c1402ed3c078953620a5bf2afc8fafca9601186e7133ca6b1bf72c35cfb"
        );
        assert_eq!(end.author, "\u{ffff}");
        assert_eq!(end.sequence_number, u64::MAX);
        assert_eq!(end.uuid, Uuid::max());
    }

    #[test]
    fn test_prefix_range_topic_author() {
        let prefix = BlobsKeyPrefix::TopicAuthor("topic".into(), "author".into());
        let start = prefix.range_start();
        let end = prefix.range_end();

        assert_eq!(start.topic_id, "topic");
        assert_eq!(start.author, "author");
        assert_eq!(start.sequence_number, 0);
        assert_eq!(start.uuid, Uuid::nil());

        assert_eq!(end.topic_id, "topic");
        assert_eq!(end.author, "author");
        assert_eq!(end.sequence_number, u64::MAX);
        assert_eq!(end.uuid, Uuid::max());
    }

    #[test]
    fn test_prefix_range_with_seq() {
        let prefix = BlobsKeyPrefix::TopicAuthorSeq("topic".into(), "author".into(), 5);
        let start = prefix.range_start();

        assert_eq!(start.topic_id, "topic");
        assert_eq!(start.author, "author");
        assert_eq!(start.sequence_number, 5);
        assert_eq!(start.uuid, Uuid::nil());
    }
}
