use redb::{Key, TableDefinition, TypeName, Value};
use std::cmp::Ordering;

#[derive(Debug, thiserror::Error)]
pub enum TopicLogKeyError {
    #[error("Topic ID contains invalid character (colon or null): {0}")]
    InvalidTopicId(String),
    #[error("Author contains invalid character (colon or null): {0}")]
    InvalidAuthor(String),
    #[error("Log ID contains invalid character (colon or null): {0}")]
    InvalidLogId(String),
}

/// Key for TOPIC_LOGS_TABLE: one row per log associated with a topic.
///
/// Binary format: `topic_id + 0x00 + author + 0x00 + log_id`, all UTF-8 with
/// no null bytes, so a direct byte comparison matches struct field ordering
/// and a topic's rows form one contiguous range.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TopicLogKey {
    // NOTE: order of these fields matters!
    pub topic_id: String,
    pub author: String,
    pub log_id: String,
}

fn valid(part: &str) -> bool {
    !part.contains(':') && !part.contains('\0')
}

impl TopicLogKey {
    pub fn new(topic_id: String, author: String, log_id: String) -> Result<Self, TopicLogKeyError> {
        if !valid(&topic_id) {
            return Err(TopicLogKeyError::InvalidTopicId(topic_id));
        }
        if !valid(&author) {
            return Err(TopicLogKeyError::InvalidAuthor(author));
        }
        if !valid(&log_id) {
            return Err(TopicLogKeyError::InvalidLogId(log_id));
        }
        Ok(Self {
            topic_id,
            author,
            log_id,
        })
    }

    /// The inclusive bounds of the range holding every log of `topic_id`.
    pub fn topic_range(topic_id: &str) -> (Self, Self) {
        let start = Self {
            topic_id: topic_id.to_string(),
            author: String::new(),
            log_id: String::new(),
        };
        // U+FFFF sorts after every valid author, so this closes the topic's range.
        let end = Self {
            topic_id: topic_id.to_string(),
            author: String::from("\u{FFFF}"),
            log_id: String::from("\u{FFFF}"),
        };
        (start, end)
    }
}

impl Value for TopicLogKey {
    type SelfType<'a> = TopicLogKey;
    type AsBytes<'a> = Vec<u8>;

    fn fixed_width() -> Option<usize> {
        None
    }

    fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
    where
        Self: 'a,
    {
        let mut parts = data.splitn(3, |&b| b == 0).map(|part| {
            std::str::from_utf8(part)
                .expect("Invalid UTF-8 in TopicLogKey")
                .to_string()
        });
        let topic_id = parts.next().expect("Missing topic_id in TopicLogKey");
        let author = parts.next().expect("Missing author in TopicLogKey");
        let log_id = parts.next().expect("Missing log_id in TopicLogKey");
        TopicLogKey {
            topic_id,
            author,
            log_id,
        }
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a>
    where
        Self: 'b,
    {
        let mut bytes = Vec::with_capacity(
            value.topic_id.len() + 1 + value.author.len() + 1 + value.log_id.len(),
        );
        bytes.extend_from_slice(value.topic_id.as_bytes());
        bytes.push(0);
        bytes.extend_from_slice(value.author.as_bytes());
        bytes.push(0);
        bytes.extend_from_slice(value.log_id.as_bytes());
        bytes
    }

    fn type_name() -> TypeName {
        TypeName::new("mailbox_server::TopicLogKey")
    }
}

impl Key for TopicLogKey {
    fn compare(data1: &[u8], data2: &[u8]) -> Ordering {
        data1.cmp(data2)
    }
}

/// Which logs belong to which topic. Blips are stored once per log (see
/// `BLIPS_TABLE`); this table is how a topic resolves to the logs to serve,
/// mirroring the topic map p2panda keeps on each node.
pub const TOPIC_LOGS_TABLE: TableDefinition<TopicLogKey, ()> =
    TableDefinition::new("topic_logs_v1");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binary_roundtrip() {
        let key = TopicLogKey::new("topic".into(), "author".into(), "log".into()).unwrap();
        let bytes = TopicLogKey::as_bytes(&key);
        assert_eq!(TopicLogKey::from_bytes(&bytes), key);
    }

    #[test]
    fn rejects_invalid_characters() {
        assert!(matches!(
            TopicLogKey::new("t:x".into(), "a".into(), "l".into()),
            Err(TopicLogKeyError::InvalidTopicId(_))
        ));
        assert!(matches!(
            TopicLogKey::new("t".into(), "a\0".into(), "l".into()),
            Err(TopicLogKeyError::InvalidAuthor(_))
        ));
        assert!(matches!(
            TopicLogKey::new("t".into(), "a".into(), "l:".into()),
            Err(TopicLogKeyError::InvalidLogId(_))
        ));
    }

    #[test]
    fn topic_range_covers_only_that_topic() {
        let (start, end) = TopicLogKey::topic_range("topic");
        let inside = TopicLogKey::new("topic".into(), "zz".into(), "zz".into()).unwrap();
        let other = TopicLogKey::new("topic2".into(), String::new(), String::new()).unwrap();
        let cmp = |a: &TopicLogKey, b: &TopicLogKey| {
            TopicLogKey::compare(&TopicLogKey::as_bytes(a), &TopicLogKey::as_bytes(b))
        };
        assert_eq!(cmp(&start, &inside), Ordering::Less);
        assert_eq!(cmp(&inside, &end), Ordering::Less);
        assert_eq!(cmp(&end, &other), Ordering::Less);
    }
}
