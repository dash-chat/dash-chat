//! Redb implementations for InboxTopic
//!
//! InboxTopic is serialized as a fixed-width array of 40 bytes:
//! - 8 bytes for the (modified) timestamp in nanoseconds
//! - 32 bytes for the topic ID
//!
//! The timestamp is stored as a big-endian 64-bit integer.
//! The topic ID is stored as a 32-byte array.
//!
//! We don't accept timestamps before 1970, so that the
//! i64 representation is always a positive value.

use super::*;

impl redb::Key for InboxTopic {
    fn compare(data1: &[u8], data2: &[u8]) -> std::cmp::Ordering {
        data1.cmp(data2)
    }
}

impl redb::Value for InboxTopic {
    type SelfType<'a>
        = InboxTopic
    where
        Self: 'a;

    type AsBytes<'a>
        = [u8; 40]
    where
        Self: 'a;

    fn fixed_width() -> Option<usize> {
        Some(40)
    }

    fn type_name() -> TypeName {
        TypeName::new("InboxTopic")
    }

    fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
    where
        Self: 'a,
    {
        let timestamp = i64::from_be_bytes(data[0..8].try_into().unwrap());
        let topic = Topic::new(data[8..40].try_into().unwrap());
        InboxTopic {
            expires_at: DateTime::from_timestamp_nanos(timestamp),
            topic,
        }
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a> {
        let mut buf = [0u8; 40];
        let timestamp = value.expires_at;
        let nanos = value
            .expires_at
            .timestamp_nanos_opt()
            .map(|n| n.max(0))
            .unwrap_or(0);
        if nanos <= 0 {
            tracing::warn!("invalid timestamp received: {timestamp}");
        }
        buf[0..8].copy_from_slice(&nanos.to_be_bytes());
        buf[8..40].copy_from_slice(&(**value.topic));
        buf
    }
}

#[cfg(test)]
#[allow(deprecated)]
mod tests {
    use super::*;

    use redb::Value;

    fn random_positive_timestamp() -> DateTime<Utc> {
        let nanos = rand::random::<i64>().max(0);
        DateTime::from_timestamp_nanos(nanos)
    }

    fn roundtrip(topic: InboxTopic) -> InboxTopic {
        let bytes = InboxTopic::as_bytes(&topic);
        InboxTopic::from_bytes(&bytes)
    }

    #[test]
    fn test_inbox_topic_roundtrip() {
        let topic = InboxTopic {
            expires_at: random_positive_timestamp(),
            topic: Topic::random().recast(),
        };
        assert_eq!(topic, roundtrip(topic.clone()));
    }

    #[test]
    fn test_inbox_topic_ordering_covariance() {
        let topic1 = roundtrip(InboxTopic {
            expires_at: DateTime::from_timestamp_nanos(rand::random()),
            topic: Topic::random().recast(),
        });
        let topic2 = roundtrip(InboxTopic {
            expires_at: DateTime::from_timestamp_nanos(rand::random()),
            topic: Topic::random().recast(),
        });
        let bytes1 = InboxTopic::as_bytes(&topic1);
        let bytes2 = InboxTopic::as_bytes(&topic2);
        let cmp_ord = topic1.cmp(&topic2);
        let cmp_bytes = bytes1.cmp(&bytes2);
        assert_eq!(
            cmp_ord, cmp_bytes,
            "

ordering is not covariant.
topic1 should be {cmp_ord:?} than topic2, but its byte representation is {cmp_bytes:?}

topic1: {topic1:#?}
topic2: {topic2:#?}

topic1 bytes: {bytes1:#?}
topic2 bytes: {bytes2:#?}",
        );
    }
}
