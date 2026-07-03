//! Public read helpers over the blips/watermarks tables, shared by the server's
//! own handlers and by replication clients (e.g. `replicating-local-mailbox-server`).

use redb::{Database, ReadableDatabase};

use crate::blip::Blip;
use crate::blips_table::{BlipsKey, BlipsKeyPrefix, BLIPS_TABLE};
use crate::watermarks_table::{WatermarksKey, WATERMARKS_TABLE};

/// The highest contiguous sequence number held locally for each author under
/// `topic`. Synchronous (opens a read transaction); wrap in `spawn_blocking`.
pub fn log_heights_for_topic(db: &Database, topic: &str) -> anyhow::Result<Vec<(String, u64)>> {
    let read_txn = db.begin_read()?;
    let table = read_txn.open_table(WATERMARKS_TABLE)?;
    // Keys are byte-ordered topic-first, so start at the topic's first author
    // and stop as soon as another topic appears.
    let start = WatermarksKey {
        topic_id: topic.to_string(),
        author: String::new(),
    };
    let mut out = Vec::new();
    for entry in table.range(start..)? {
        let (key, value) = entry?;
        let key: WatermarksKey = key.value();
        if key.topic_id != topic {
            break;
        }
        out.push((key.author, value.value()));
    }
    Ok(out)
}

/// All stored blips for `(topic, author)` whose sequence number is `>= from`,
/// ordered by sequence number. Synchronous; wrap in `spawn_blocking`.
pub fn blips_since(
    db: &Database,
    topic: &str,
    author: &str,
    from: u64,
) -> anyhow::Result<Vec<(BlipsKey, Blip)>> {
    let read_txn = db.begin_read()?;
    let table = read_txn.open_table(BLIPS_TABLE)?;
    let start =
        BlipsKeyPrefix::TopicAuthorSeq(topic.to_string(), author.to_string(), from).range_start();
    let end = BlipsKeyPrefix::TopicAuthor(topic.to_string(), author.to_string()).range_end();
    let mut out = Vec::new();
    for entry in table.range(start..=end)? {
        let (key, value) = entry?;
        out.push((key.value(), Blip::from(value.value().to_vec())));
    }
    Ok(out)
}
