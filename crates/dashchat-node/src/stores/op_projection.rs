use aliased::Aliasing;
use derive_more::derive::{Deref, From};
use p2panda::Hash;
use p2panda::operation::Header;
use p2panda::streams::ProcessedOperation;
use p2panda_auth::group::GroupAction;
use p2panda_auth::processor::GroupsArgs;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::{
    AckedOp, AnnouncementsPayload, ChatId, ChatPayload, DeviceGroupPayload, InboxPayload, Payload,
    Topic,
};
use crate::{
    AgentId, DeleteCandidate, DeviceId, Profile, SystemNotification, TopicId, forward_edit_closure,
};

// TODO: rework this not as migrations, but as a single schema that, when changed,
//       triggers a re-projection of the db.
const MIGRATIONS: &[&str] = &[
    "CREATE TABLE IF NOT EXISTS devices (
        device_id BLOB PRIMARY KEY,
        agent_id BLOB NOT NULL
    )",
    "CREATE TABLE IF NOT EXISTS agents (
        agent_id BLOB PRIMARY KEY,
        profile BLOB NULL,
        blocked BOOLEAN NOT NULL DEFAULT FALSE
    )",
    "CREATE TABLE IF NOT EXISTS subscribed_topics (
        topic_id BLOB PRIMARY KEY
    )",
    "CREATE TABLE IF NOT EXISTS active_inboxes (
        topic_id BLOB NOT NULL PRIMARY KEY,
        expires_at_nanos INTEGER NOT NULL
    )",
    "CREATE TABLE IF NOT EXISTS group_chats (
        chat_id BLOB NOT NULL PRIMARY KEY
    )",
    "CREATE TABLE IF NOT EXISTS tombstones (
        topic_id BLOB NOT NULL,
        op_hash BLOB NOT NULL,
        reason TEXT NOT NULL,
        PRIMARY KEY (topic_id, op_hash)
    )",
    "CREATE TABLE IF NOT EXISTS chat_log_heads (
        topic_id BLOB NOT NULL,
        author_device_id BLOB NOT NULL,
        seq_num INTEGER NOT NULL,
        op_hash BLOB NOT NULL,
        PRIMARY KEY (topic_id, author_device_id)
    )",
    "CREATE TABLE IF NOT EXISTS message_acks (
        topic_id BLOB NOT NULL,
        acker_device_id BLOB NOT NULL,
        author_device_id BLOB NOT NULL,
        seq_num INTEGER NOT NULL,
        op_hash BLOB NOT NULL,
        PRIMARY KEY (topic_id, acker_device_id, author_device_id)
    )",
];

/// Why an operation was tombstoned.
//
// TODO: ACID: The tombstone state for `DeletedForMe` is actually required for
// full reconstruction of the OpProjection, because when operations are dropped,
// there is nothing left to establish the edit chain and to know to transitively
// tombstone new Edits that may come in. This means that purging the OpProjection
// and replaying the operation streams is not sufficient to restore the OpProjection.
// Possible solutions include:
// - Storing EditMessage references on the Header as a custom extension (requires p2panda support)
// - Persisting either edit chains or tombstones in a non-purgable store so that it can be used for projection reconstruction
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TombstoneReason {
    DeletedForEveryone,
    DeletedForMe,
}

impl TombstoneReason {
    fn to_db(self) -> String {
        serde_json::to_value(self)
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned))
            .expect("TombstoneReason serializes to a string")
    }

    fn from_db(value: &str) -> anyhow::Result<Self> {
        Ok(serde_json::from_value(serde_json::Value::String(
            value.to_owned(),
        ))?)
    }
}

/// The [`OpProjection`] is a projection of the [`crate::stores::OpStore`] that is used to make streamlined queries.
/// It only contains data already present in the operations, just reshaped to be more queryable.
///
/// - Writes only occur in the [`Self::reduce`] method.
/// - The projection is populated by streaming operations through [`Self::reduce`].
/// - The projection can be purged and rebuilt by replaying operation streams.
/// - If a log is partially purged, the OpProjection can be deleted and the streams replayed from their new starting point.
/// - To achieve ACID compliance:
///   - every write must be idempotent in case operations need to be replayed.
///   - every write must be deterministic: e.g. a write cannot depend on [`crate::stores::LocalStore`] state.
#[derive(Clone)]
pub struct OpProjection {
    pool: SqlitePool,
}

impl OpProjection {
    pub async fn new(pool: SqlitePool) -> anyhow::Result<Self> {
        for sql in MIGRATIONS {
            sqlx::query(sql).execute(&pool).await?;
        }

        let projection = Self { pool };
        Ok(projection)
    }

    pub async fn all_contact_agent_ids(&self) -> anyhow::Result<BTreeSet<AgentId>> {
        let rows: Vec<(AgentId,)> = sqlx::query_as("SELECT agent_id FROM agents")
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().map(|(id,)| id).collect())
    }

    pub async fn lookup_contact_by_device_id(
        &self,
        device_id: DeviceId,
    ) -> anyhow::Result<Option<AgentId>> {
        let row: Option<(AgentId,)> =
            sqlx::query_as("SELECT agent_id FROM devices WHERE device_id = ?")
                .bind(device_id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.map(|(id,)| id))
    }

    /// Look up the device ID for a given agent ID.
    ///
    /// This is temporary, and will not be needed once device gropus are
    /// implemented and [ChatMember] becomes [AgentId].
    pub async fn lookup_contact_by_agent_id(
        &self,
        agent_id: AgentId,
    ) -> anyhow::Result<Option<DeviceId>> {
        let row: Option<(DeviceId,)> =
            sqlx::query_as("SELECT device_id FROM devices WHERE agent_id = ?")
                .bind(agent_id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.map(|(id,)| id))
    }

    /// Look up every device known to belong to a given agent.
    pub async fn lookup_devices_by_agent_id(
        &self,
        agent_id: AgentId,
    ) -> anyhow::Result<Vec<DeviceId>> {
        let rows: Vec<(DeviceId,)> =
            sqlx::query_as("SELECT device_id FROM devices WHERE agent_id = ?")
                .bind(agent_id)
                .fetch_all(&self.pool)
                .await?;
        Ok(rows.into_iter().map(|(id,)| id).collect())
    }

    /// Look up multiple contacts in a single query.
    ///
    /// Returns a map from `DeviceId` to its `AgentId`. Devices that have no
    /// contact entry are simply absent from the map — the caller can compare
    /// against the input slice to find which lookups missed.
    pub async fn lookup_contacts(
        &self,
        device_ids: impl IntoIterator<Item = &DeviceId>,
    ) -> anyhow::Result<HashMap<DeviceId, AgentId>> {
        let device_ids = device_ids.into_iter().collect::<Vec<_>>();
        if device_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let placeholders = std::iter::repeat("?")
            .take(device_ids.len())
            .collect::<Vec<_>>()
            .join(", ");
        let sql =
            format!("SELECT device_id, agent_id FROM devices WHERE device_id IN ({placeholders})");
        let mut q = sqlx::query_as::<_, (DeviceId, AgentId)>(&sql);
        for id in device_ids {
            q = q.bind(*id);
        }
        Ok(q.fetch_all(&self.pool).await?.into_iter().collect())
    }

    pub async fn is_author_blocked(&self, device_id: &DeviceId) -> anyhow::Result<bool> {
        let row: Option<(bool,)> = sqlx::query_as(
            "
            SELECT blocked FROM agents
            JOIN devices ON agents.agent_id = devices.agent_id
            WHERE devices.device_id = ?
            ",
        )
        .bind(device_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|(blocked,)| blocked).unwrap_or(false))
    }

    pub async fn get_profile(&self, agent_id: AgentId) -> anyhow::Result<Option<Profile>> {
        let row: Option<(Option<Profile>,)> =
            sqlx::query_as("SELECT profile FROM agents WHERE agent_id = ?")
                .bind(agent_id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.and_then(|(profile,)| profile))
    }

    pub async fn get_group_chat_ids(&self) -> anyhow::Result<Vec<ChatId>> {
        let rows: Vec<(Topic<crate::topic::kind::Untyped>,)> =
            sqlx::query_as("SELECT chat_id FROM group_chats")
                .fetch_all(&self.pool)
                .await?;
        rows.into_iter()
            .map(|(id,)| Topic::<crate::topic::kind::Chat>::from_topic_id(crate::TopicId::from(id)))
            .collect()
    }

    /// Per author of `topic`, the highest operation acked by any device of a
    /// *different* agent (devices with an unknown agent mapping count as
    /// different). This is what drives the "delivered" message status.
    pub async fn delivered_acks(
        &self,
        topic: TopicId,
    ) -> anyhow::Result<BTreeMap<DeviceId, AckedOp>> {
        let rows: Vec<(DeviceId, i64, Vec<u8>)> = sqlx::query_as(
            "
            SELECT ma.author_device_id, ma.seq_num, ma.op_hash
            FROM message_acks ma
            LEFT JOIN devices acker ON acker.device_id = ma.acker_device_id
            LEFT JOIN devices author ON author.device_id = ma.author_device_id
            WHERE ma.topic_id = ?
              AND (acker.agent_id IS NULL
                   OR author.agent_id IS NULL
                   OR acker.agent_id != author.agent_id)
            ",
        )
        .bind(topic.as_bytes().to_vec())
        .fetch_all(&self.pool)
        .await?;

        let mut acks: BTreeMap<DeviceId, AckedOp> = BTreeMap::new();
        for (author, seq, hash) in rows {
            let acked = AckedOp {
                hash: hash_from_db(hash)?,
                seq: seq as u64,
            };
            match acks.entry(author) {
                std::collections::btree_map::Entry::Vacant(e) => {
                    e.insert(acked);
                }
                std::collections::btree_map::Entry::Occupied(mut e) => {
                    if acked.seq > e.get().seq {
                        e.insert(acked);
                    }
                }
            }
        }
        Ok(acks)
    }

    /// The entries a new [`ChatPayload::MessageAck`] authored by `me` should
    /// contain: per author (excluding `me`), the latest processed non-ack chat
    /// operation, but only where it is newer than what `me` has already acked.
    pub async fn ack_delta(
        &self,
        topic: TopicId,
        me: DeviceId,
    ) -> anyhow::Result<BTreeMap<DeviceId, AckedOp>> {
        let rows: Vec<(DeviceId, i64, Vec<u8>)> = sqlx::query_as(
            "
            SELECT h.author_device_id, h.seq_num, h.op_hash
            FROM chat_log_heads h
            LEFT JOIN message_acks ma
              ON ma.topic_id = h.topic_id
              AND ma.author_device_id = h.author_device_id
              AND ma.acker_device_id = ?
            WHERE h.topic_id = ?
              AND h.author_device_id != ?
              AND (ma.seq_num IS NULL OR h.seq_num > ma.seq_num)
            ",
        )
        .bind(me)
        .bind(topic.as_bytes().to_vec())
        .bind(me)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|(author, seq, hash)| {
                Ok((
                    author,
                    AckedOp {
                        hash: hash_from_db(hash)?,
                        seq: seq as u64,
                    },
                ))
            })
            .collect()
    }

    /// Every topic that has recorded chat log heads, i.e. every chat topic that
    /// may need a [`ChatPayload::MessageAck`] published.
    pub async fn ack_topic_ids(&self) -> anyhow::Result<Vec<TopicId>> {
        let rows: Vec<(Topic<crate::topic::kind::Untyped>,)> =
            sqlx::query_as("SELECT DISTINCT topic_id FROM chat_log_heads")
                .fetch_all(&self.pool)
                .await?;
        Ok(rows.into_iter().map(|(id,)| TopicId::from(id)).collect())
    }

    pub async fn is_tombstoned(&self, topic: TopicId, op_hash: Hash) -> anyhow::Result<bool> {
        let row: Option<(i64,)> =
            sqlx::query_as("SELECT 1 FROM tombstones WHERE topic_id = ? AND op_hash = ?")
                .bind(topic.as_bytes().to_vec())
                .bind(op_hash.as_bytes().to_vec())
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.is_some())
    }

    pub async fn tombstoned_hashes(&self, topic: TopicId) -> anyhow::Result<BTreeSet<Hash>> {
        let rows: Vec<(Vec<u8>,)> =
            sqlx::query_as("SELECT op_hash FROM tombstones WHERE topic_id = ?")
                .bind(topic.as_bytes().to_vec())
                .fetch_all(&self.pool)
                .await?;
        rows.into_iter()
            .map(|(bytes,)| {
                let arr: [u8; 32] = bytes
                    .try_into()
                    .map_err(|_| anyhow::anyhow!("tombstone op_hash is not 32 bytes"))?;
                Ok(Hash::from_bytes(arr))
            })
            .collect()
    }

    /// The reason `op_hash` was tombstoned in `topic`, or `None` if it isn't
    /// tombstoned.
    pub async fn tombstone_reason(
        &self,
        topic: TopicId,
        op_hash: Hash,
    ) -> anyhow::Result<Option<TombstoneReason>> {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT reason FROM tombstones WHERE topic_id = ? AND op_hash = ?")
                .bind(topic.as_bytes().to_vec())
                .bind(op_hash.as_bytes().to_vec())
                .fetch_optional(&self.pool)
                .await?;
        row.map(|(reason,)| TombstoneReason::from_db(&reason))
            .transpose()
    }

    /// Every tombstone in `topic`, paired with its reason. The frontend uses
    /// this to drop delete-for-me messages (and their edits) from view while
    /// keeping the delete-for-everyone placeholders.
    pub async fn tombstones(
        &self,
        topic: TopicId,
    ) -> anyhow::Result<HashMap<Hash, TombstoneReason>> {
        let rows: Vec<(Vec<u8>, String)> =
            sqlx::query_as("SELECT op_hash, reason FROM tombstones WHERE topic_id = ?")
                .bind(topic.as_bytes().to_vec())
                .fetch_all(&self.pool)
                .await?;
        rows.into_iter()
            .map(|(bytes, reason)| {
                let arr: [u8; 32] = bytes
                    .try_into()
                    .map_err(|_| anyhow::anyhow!("tombstone op_hash is not 32 bytes"))?;
                Ok((Hash::from_bytes(arr), TombstoneReason::from_db(&reason)?))
            })
            .collect()
    }

    pub async fn reduce(
        &self,
        me: AgentId,
        operation: &ProcessedOperation<Payload>,
        // XXX: once refined operation logs (e.g. `all_valid_ops`) are moved
        //      to the projection layer, this Node injection must be removed.
        node: BadUseOfNode,
    ) -> Result<Option<SystemNotification>, ProjectionError> {
        let author = DeviceId::from(operation.author());
        let payload = operation.message();
        let topic = operation.topic();

        self.enforce_blocklist(operation).await?;

        let event = match &payload {
            Payload::Chat(ChatPayload::MessageAck { acks }) => {
                let mut delivered = BTreeMap::new();
                for (acked_author, acked) in acks {
                    let changed = self
                        .record_message_ack(topic, author, *acked_author, *acked)
                        .await?;
                    if changed && self.ack_counts_as_delivered(author, *acked_author).await? {
                        delivered.insert(*acked_author, *acked);
                    }
                }
                (!delivered.is_empty()).then_some(SystemNotification::MessageAcks {
                    topic,
                    acks: delivered,
                })
            }

            Payload::Chat(ChatPayload::IntroduceAgents { agents }) => {
                for (device_id, agent_id) in agents {
                    self.save_agent_mapping(*device_id, *agent_id).await?;
                }
                None
            }

            Payload::Chat(ChatPayload::DeleteMessage { hashes }) => {
                self.validate_delete(topic, operation.event.operation.header(), hashes, node)
                    .await?;
                for hash in hashes {
                    self.add_tombstone(topic.into(), *hash, TombstoneReason::DeletedForEveryone)
                        .await?;
                }
                Some(SystemNotification::Tombstones {
                    topic: topic.into(),
                    hashes: hashes.clone(),
                    reason: TombstoneReason::DeletedForEveryone,
                })
            }

            Payload::Chat(ChatPayload::EditMessage { edit_hash, .. }) => {
                // An edit of an already-tombstoned message is itself tombstoned,
                // inheriting the referent's reason. This is how a delete-for-me
                // (which only names the original message) reaches edits that
                // arrive after the delete, and it keeps a delete-for-everyone's
                // late edits from lingering too. Edits of live messages are
                // validated later in `process_app`.
                if let Some(reason) = self.tombstone_reason(topic.into(), *edit_hash).await? {
                    let self_hash = operation.event.operation.header().hash();
                    self.add_tombstone(topic.into(), self_hash, reason).await?;
                    Some(SystemNotification::Tombstones {
                        topic: topic.into(),
                        hashes: BTreeSet::from_iter([*edit_hash]),
                        reason,
                    })
                } else {
                    None
                }
            }

            Payload::Announcements(AnnouncementsPayload::SetProfile(profile)) => {
                // HACK: The announcements topic id IS the agent_id bytes, so we can reconstruct it here.
                let agent_id =
                    AgentId::from(crate::ActorId::from_bytes(topic.as_bytes()).map_err(|e| {
                        anyhow::anyhow!("invalid agent_id bytes in announcements topic: {e}")
                    })?);

                tracing::info!(me = ?me.aliased(), agent_id = ?agent_id.aliased(), ?profile, "save_profile");

                self.save_profile(agent_id, profile.clone()).await?;
                None
            }

            Payload::DeviceGroup(p) => match p {
                DeviceGroupPayload::AddContact { agent_id, .. } => {
                    self.save_agent_mapping(author, *agent_id).await?;
                    None
                }
                DeviceGroupPayload::BlockAgent(agent_id) => {
                    self.block_agent(*agent_id).await?;
                    None
                }
                DeviceGroupPayload::UnblockAgent(agent_id) => {
                    self.unblock_agent(*agent_id).await?;
                    None
                }
                DeviceGroupPayload::DeleteForMe(delete) => {
                    let hashes = self
                        .tombstone_message_for_me(delete.chat_id, delete.message_hash, node)
                        .await?;
                    Some(SystemNotification::Tombstones {
                        topic: delete.chat_id.into(),
                        hashes,
                        reason: TombstoneReason::DeletedForMe,
                    })
                }
                _ => None,
            },

            // ACID: TODO: it's not correct to unconditionally save contact info here.
            //             This needs to be limited to only accepted requests.
            Payload::Inbox(InboxPayload::ContactRequest {
                agent_id, profile, ..
            })
            | Payload::Inbox(InboxPayload::ContactRequestAccept { agent_id, profile }) => {
                self.save_agent_mapping(author, *agent_id).await?;
                self.save_profile(*agent_id, profile.clone()).await?;
                None
            }

            // We define group chats as topics which contain a CreateGroup that makes at least
            // one member an admin.
            //
            // 1:1 chats are also group chats, but both members have only Write access,
            // meaning nobody will ever have admin access.
            //
            // TODO: this needs to be much more clearly defined, see https://hackmd.io/1S2xtZfXTo6N5WinzCnqWw
            Payload::GroupControl(GroupsArgs { action, .. }) => {
                match action {
                    GroupAction::Create { initial_members } => {
                        for (_, access) in initial_members {
                            if *access == p2panda_auth::Access::manage() {
                                self.mark_group_as_group_chat(ChatId::from_topic_id(topic)?)
                                    .await?;
                                break;
                            }
                        }
                    }
                    _ => (),
                };
                None
            }

            _ => {
                // Nothing to do.
                None
            }
        };

        if let Payload::Chat(chat_payload) = &payload {
            if !matches!(chat_payload, ChatPayload::MessageAck { .. }) {
                let header = operation.event.operation.header();
                self.record_chat_log_head(topic, author, header.seq_num, header.hash())
                    .await?;
            }
        }

        Ok(event)
    }

    // === helpers === //

    /// While the contact is blocked, invalidate all operations from them except for the necessary ones.
    async fn enforce_blocklist(
        &self,
        operation: &ProcessedOperation<Payload>,
    ) -> Result<(), ProjectionError> {
        let author = DeviceId::from(operation.author());
        if self.is_author_blocked(&author).await? {
            let allow = match operation.message() {
                // Group control messages are necessary to maintain group chats.
                Payload::GroupControl(_) => true,
                // Group info messages are necessary to maintain group chats.
                Payload::Chat(ChatPayload::GroupInfo(_)) => true,
                // All other operations are invalid.
                _ => false,
            };
            if !allow {
                return Err(ProjectionError::invalid(format!(
                    "author {author} is blocked"
                )));
            }
        };
        Ok(())
    }

    async fn validate_delete(
        &self,
        topic: TopicId,
        header: &Header,
        payload: &BTreeSet<Hash>,
        node: BadUseOfNode,
    ) -> Result<(), ProjectionError> {
        let hash = header.hash();
        let author = DeviceId::from(header.verifying_key);

        // On replay (or a duplicate delivery) the delete has already
        // been applied and its targets' bodies are gone, which would
        // fail validation; skip silently instead.
        let mut already_applied = true;
        for h in payload.iter() {
            if !self.is_tombstoned(topic, *h).await? {
                already_applied = false;
                break;
            }
        }
        if already_applied {
            return Err(ProjectionError::invalid(format!(
                "delete has already been applied: {:?}",
                hash.aliased()
            )));
        }

        // Authorship: a delete may only tombstone operations authored by the
        // same agent as the deleter. Check every *target* op (the hashes in the
        // payload), not the delete op itself. Body-less/tombstoned copies still
        // carry the author's key in their header, so late joiners can enforce
        // this too.
        //
        // TODO: Targets we haven't synced yet can't be checked here.
        // This is only applicable for cross-device deletes,
        // and is fixed once we have custom processors for partial ordering.
        //
        // TODO: Needs to be multi-device aware
        for target in payload.iter() {
            let Some(target_op) = node.op_store.get_operation(target).await? else {
                continue;
            };
            let target_device = DeviceId::from(target_op.header.verifying_key);
            if target_device != author {
                tracing::warn!(op = ?hash.aliased(), target = ?target.aliased(), "delete references another author's operation; skipping");
                return Err(ProjectionError::invalid(format!(
                    "delete references another author's operation: {:?} != {:?}",
                    target_device.aliased(),
                    author.aliased()
                )));
            }
        }

        let chat_id = ChatId::from_topic_id(topic)?;
        let valid_ops = node.valid_chat_ops(chat_id).await?;
        let delete_ts: u64 = header.timestamp.into();
        if let Err(err) = (DeleteCandidate {
            hashes: payload.clone(),
            deleter: author,
            delete_timestamp: delete_ts,
            self_hash: Some(hash),
        })
        .validate(&valid_ops)
        {
            tracing::warn!(?err, op = ?hash.aliased(), "ignoring invalid delete message");
            return Err(ProjectionError::invalid(format!(
                "invalid delete message: {err}"
            )));
        }

        Ok(())
    }

    // === setters === //

    async fn save_agent_mapping(
        &self,
        device_id: DeviceId,
        agent_id: AgentId,
    ) -> anyhow::Result<()> {
        sqlx::query("INSERT OR IGNORE INTO devices (device_id, agent_id) VALUES (?, ?)")
            .bind(device_id)
            .bind(agent_id)
            .execute(&self.pool)
            .await?;

        sqlx::query("INSERT OR IGNORE INTO agents (agent_id) VALUES (?)")
            .bind(agent_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn save_profile(&self, agent_id: AgentId, profile: Profile) -> anyhow::Result<()> {
        sqlx::query(
            "
            INSERT INTO agents (agent_id, profile) VALUES (?, ?) 
            ON CONFLICT(agent_id) DO UPDATE SET profile = excluded.profile
        ",
        )
        .bind(agent_id)
        .bind(profile)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn block_agent(&self, agent_id: AgentId) -> anyhow::Result<()> {
        sqlx::query("UPDATE agents SET blocked = TRUE WHERE agent_id = ?")
            .bind(agent_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn unblock_agent(&self, agent_id: AgentId) -> anyhow::Result<()> {
        sqlx::query("UPDATE agents SET blocked = FALSE WHERE agent_id = ?")
            .bind(agent_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn mark_group_as_group_chat(&self, chat_id: ChatId) -> anyhow::Result<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("INSERT OR IGNORE INTO group_chats (chat_id) VALUES (?)")
            .bind(chat_id.to_vec())
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Tombstone `root` and its entire current edit chain in `chat_id` with
    /// [`TombstoneReason::DeletedForMe`], returning the hashes tombstoned so
    /// callers can notify the frontend of all of them (not just `root`).
    async fn tombstone_message_for_me(
        &self,
        chat_id: ChatId,
        root: Hash,
        node: BadUseOfNode,
    ) -> anyhow::Result<BTreeSet<Hash>> {
        let chat_topic: TopicId = chat_id.into();
        let valid_ops = node.valid_chat_ops(chat_id).await?;

        let hashes = forward_edit_closure(&valid_ops, root);
        for hash in &hashes {
            self.add_tombstone(chat_topic, *hash, TombstoneReason::DeletedForMe)
                .await?;
        }
        Ok(hashes)
    }

    /// Record the latest processed non-ack chat operation per (topic, author).
    /// Monotonic upsert: an existing row is only replaced by a higher seq_num,
    /// so replays are idempotent.
    async fn record_chat_log_head(
        &self,
        topic: TopicId,
        author: DeviceId,
        seq_num: u64,
        op_hash: Hash,
    ) -> anyhow::Result<()> {
        sqlx::query(
            "INSERT INTO chat_log_heads (topic_id, author_device_id, seq_num, op_hash)
             VALUES (?, ?, ?, ?)
             ON CONFLICT (topic_id, author_device_id) DO UPDATE SET
                 seq_num = excluded.seq_num,
                 op_hash = excluded.op_hash
             WHERE excluded.seq_num > chat_log_heads.seq_num",
        )
        .bind(topic.as_bytes().to_vec())
        .bind(author)
        .bind(seq_num as i64)
        .bind(op_hash.as_bytes().to_vec())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Whether an ack by `acker` counts toward "delivered" for ops authored by
    /// `author`: the two devices must not belong to the same agent, with
    /// unknown mappings counting as different — mirroring
    /// [`Self::delivered_acks`].
    async fn ack_counts_as_delivered(
        &self,
        acker: DeviceId,
        author: DeviceId,
    ) -> anyhow::Result<bool> {
        let acker_agent = self.lookup_contact_by_device_id(acker).await?;
        let author_agent = self.lookup_contact_by_device_id(author).await?;
        Ok(match (acker_agent, author_agent) {
            (Some(acker_agent), Some(author_agent)) => acker_agent != author_agent,
            _ => true,
        })
    }

    /// Fold one [`ChatPayload::MessageAck`] entry into the per-acker ack map.
    /// Monotonic upsert like [`Self::record_chat_log_head`]. Returns whether
    /// the stored state changed.
    async fn record_message_ack(
        &self,
        topic: TopicId,
        acker: DeviceId,
        author: DeviceId,
        acked: AckedOp,
    ) -> anyhow::Result<bool> {
        let result = sqlx::query(
            "INSERT INTO message_acks (topic_id, acker_device_id, author_device_id, seq_num, op_hash)
             VALUES (?, ?, ?, ?, ?)
             ON CONFLICT (topic_id, acker_device_id, author_device_id) DO UPDATE SET
                 seq_num = excluded.seq_num,
                 op_hash = excluded.op_hash
             WHERE excluded.seq_num > message_acks.seq_num",
        )
        .bind(topic.as_bytes().to_vec())
        .bind(acker)
        .bind(author)
        .bind(acked.seq as i64)
        .bind(acked.hash.as_bytes().to_vec())
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Record an operation hash in the per-topic tombstone set with the reason
    /// it was tombstoned.
    ///
    /// Reasons are first-write-wins, with one deliberate
    /// exception: a `DeletedForMe` upgrades an existing `DeletedForEveryone` (so
    /// a message I deleted for myself vanishes even when it's also deleted for
    /// everyone). Every other combination keeps the existing reason.
    async fn add_tombstone(
        &self,
        topic: TopicId,
        op_hash: Hash,
        reason: TombstoneReason,
    ) -> anyhow::Result<()> {
        sqlx::query(
            "INSERT INTO tombstones (topic_id, op_hash, reason) VALUES (?, ?, ?)
             ON CONFLICT(topic_id, op_hash) DO UPDATE SET reason = excluded.reason
             WHERE excluded.reason = ? AND tombstones.reason = ?",
        )
        .bind(topic.as_bytes().to_vec())
        .bind(op_hash.as_bytes().to_vec())
        .bind(reason.to_db())
        // Only the specific DeletedForEveryone -> DeletedForMe upgrade replaces
        // an existing reason.
        .bind(TombstoneReason::DeletedForMe.to_db())
        .bind(TombstoneReason::DeletedForEveryone.to_db())
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

fn hash_from_db(bytes: Vec<u8>) -> anyhow::Result<Hash> {
    let arr: [u8; 32] = bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("op_hash is not 32 bytes"))?;
    Ok(Hash::from_bytes(arr))
}

#[derive(Debug)]
pub enum ProjectionError {
    InvalidOp(String),
    Any(anyhow::Error),
}

impl ProjectionError {
    pub fn invalid(msg: impl Into<String>) -> Self {
        Self::InvalidOp(msg.into())
    }
}

impl From<anyhow::Error> for ProjectionError {
    fn from(error: anyhow::Error) -> Self {
        Self::Any(error)
    }
}

#[derive(Clone, Deref, From)]
#[deprecated = "XXX: this is temporary only until we properly implement projections of operations. Until then we grab data directly from the node."]
pub struct BadUseOfNode(crate::Node);

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::{stores::create_sqlite_pool, topic::kind};

    fn agent(n: u8) -> AgentId {
        AgentId::from(crate::ActorId::from(
            p2panda::SigningKey::from_bytes(&[n; 32]).verifying_key(),
        ))
    }

    fn device(n: u8) -> DeviceId {
        DeviceId::from(p2panda::SigningKey::from_bytes(&[n; 32]).verifying_key())
    }

    async fn projection() -> OpProjection {
        let dir = tempfile::tempdir().unwrap();
        let pool = create_sqlite_pool(dir.path().join("op_projection.db"))
            .await
            .unwrap();
        // Keep the tempdir alive for the duration of the pool by leaking it; the
        // OS reclaims it when the test process exits.
        std::mem::forget(dir);
        OpProjection::new(pool).await.unwrap()
    }

    /// Blocking an agent blocks every device that maps to it, and unblocking
    /// restores every device.
    #[tokio::test]
    async fn test_block_applies_to_all_devices_of_agent() {
        let db = projection().await;

        let agent_a = agent(1);
        let agent_b = agent(2);
        let (d1, d2, d3) = (device(10), device(11), device(20));

        // agent_a controls two devices, agent_b controls one.
        db.save_agent_mapping(d1, agent_a).await.unwrap();
        db.save_agent_mapping(d2, agent_a).await.unwrap();
        db.save_agent_mapping(d3, agent_b).await.unwrap();

        // Nothing blocked initially.
        assert!(!db.is_author_blocked(&d1).await.unwrap());
        assert!(!db.is_author_blocked(&d2).await.unwrap());
        assert!(!db.is_author_blocked(&d3).await.unwrap());

        db.block_agent(agent_a).await.unwrap();

        // Both of agent_a's devices are blocked; agent_b's device is not.
        assert!(db.is_author_blocked(&d1).await.unwrap());
        assert!(db.is_author_blocked(&d2).await.unwrap());
        assert!(!db.is_author_blocked(&d3).await.unwrap());

        db.unblock_agent(agent_a).await.unwrap();

        // Unblocking restores every device of the agent.
        assert!(!db.is_author_blocked(&d1).await.unwrap());
        assert!(!db.is_author_blocked(&d2).await.unwrap());
        assert!(!db.is_author_blocked(&d3).await.unwrap());
    }

    /// A device with no contact entry is never considered blocked.
    #[tokio::test]
    async fn test_unknown_device_is_not_blocked() {
        let db = projection().await;
        assert!(!db.is_author_blocked(&device(99)).await.unwrap());
    }

    /// A device newly mapped to an already-blocked agent is immediately blocked,
    /// since the block lives on the agent, not the device.
    #[tokio::test]
    async fn test_block_covers_devices_added_after_block() {
        let db = projection().await;
        let agent_a = agent(1);

        db.save_agent_mapping(device(10), agent_a).await.unwrap();
        db.block_agent(agent_a).await.unwrap();

        // A second device shows up for the same agent after the block.
        db.save_agent_mapping(device(11), agent_a).await.unwrap();

        assert!(db.is_author_blocked(&device(11)).await.unwrap());
    }

    /// `all_contact_agent_ids` returns one entry per agent, even when an agent
    /// has multiple devices, and includes blocked agents.
    #[tokio::test]
    async fn test_all_contact_agent_ids_dedups_by_agent() {
        let db = projection().await;

        let agent_a = agent(1);
        let agent_b = agent(2);

        db.save_agent_mapping(device(10), agent_a).await.unwrap();
        db.save_agent_mapping(device(11), agent_a).await.unwrap();
        db.save_agent_mapping(device(20), agent_b).await.unwrap();

        db.block_agent(agent_b).await.unwrap();

        assert_eq!(
            db.all_contact_agent_ids().await.unwrap(),
            maplit::btreeset![agent_a, agent_b]
        );
    }

    /// Chat log heads only ever move forward: a replayed (older) operation
    /// never overwrites a newer head.
    #[tokio::test]
    async fn test_chat_log_heads_monotonic() {
        let db = projection().await;
        let topic = *Topic::<kind::Untyped>::new([1; 32]);
        let author = device(10);

        let h3 = Hash::digest(b"op3");
        let h5 = Hash::digest(b"op5");

        db.record_chat_log_head(topic, author, 3, h3).await.unwrap();
        // Replay of an older op is ignored.
        db.record_chat_log_head(topic, author, 1, Hash::digest(b"op1"))
            .await
            .unwrap();
        let delta = db.ack_delta(topic, device(99)).await.unwrap();
        assert_eq!(delta.get(&author), Some(&AckedOp { hash: h3, seq: 3 }));

        db.record_chat_log_head(topic, author, 5, h5).await.unwrap();
        let delta = db.ack_delta(topic, device(99)).await.unwrap();
        assert_eq!(delta.get(&author), Some(&AckedOp { hash: h5, seq: 5 }));

        assert_eq!(db.ack_topic_ids().await.unwrap(), vec![topic]);
    }

    /// `record_message_ack` reports whether stored state changed, folding
    /// monotonically per (acker, author).
    #[tokio::test]
    async fn test_record_message_ack_change_detection() {
        let db = projection().await;
        let topic = *Topic::<kind::Untyped>::new([1; 32]);
        let (acker, author) = (device(10), device(20));

        let acked = AckedOp {
            hash: Hash::digest(b"a"),
            seq: 2,
        };
        assert!(
            db.record_message_ack(topic, acker, author, acked)
                .await
                .unwrap()
        );
        // Replay of the same ack: no change.
        assert!(
            !db.record_message_ack(topic, acker, author, acked)
                .await
                .unwrap()
        );
        // An older ack never regresses the fold.
        assert!(
            !db.record_message_ack(
                topic,
                acker,
                author,
                AckedOp {
                    hash: Hash::digest(b"old"),
                    seq: 1,
                }
            )
            .await
            .unwrap()
        );
        // A newer one advances it.
        assert!(
            db.record_message_ack(
                topic,
                acker,
                author,
                AckedOp {
                    hash: Hash::digest(b"new"),
                    seq: 7,
                }
            )
            .await
            .unwrap()
        );
    }

    /// `ack_delta` is the per-author gap between the latest processed non-ack
    /// op and what `me` has already acked; `delivered_acks` folds every other
    /// acker, skipping devices of the author's own agent.
    #[tokio::test]
    async fn test_ack_delta_and_delivered_acks() {
        let db = projection().await;
        let topic = *Topic::<kind::Untyped>::new([1; 32]);

        let me = device(1);
        // Author with two devices under one agent, plus an unrelated author.
        let agent_a = agent(2);
        let (author_a, sibling_a) = (device(20), device(21));
        let author_b = device(30);
        db.save_agent_mapping(author_a, agent_a).await.unwrap();
        db.save_agent_mapping(sibling_a, agent_a).await.unwrap();
        db.save_agent_mapping(author_b, agent(3)).await.unwrap();

        let head_a = AckedOp {
            hash: Hash::digest(b"a4"),
            seq: 4,
        };
        let head_b = AckedOp {
            hash: Hash::digest(b"b2"),
            seq: 2,
        };
        db.record_chat_log_head(topic, author_a, head_a.seq, head_a.hash)
            .await
            .unwrap();
        db.record_chat_log_head(topic, author_b, head_b.seq, head_b.hash)
            .await
            .unwrap();
        // My own head must never appear in my delta.
        db.record_chat_log_head(topic, me, 9, Hash::digest(b"mine"))
            .await
            .unwrap();

        let delta = db.ack_delta(topic, me).await.unwrap();
        assert_eq!(
            delta,
            maplit::btreemap![author_a => head_a, author_b => head_b]
        );

        // Once I've acked author_a, only author_b remains in the delta.
        db.record_message_ack(topic, me, author_a, head_a)
            .await
            .unwrap();
        let delta = db.ack_delta(topic, me).await.unwrap();
        assert_eq!(delta, maplit::btreemap![author_b => head_b]);

        // Delivered: my ack of author_a counts (different agent)...
        let delivered = db.delivered_acks(topic).await.unwrap();
        assert_eq!(delivered, maplit::btreemap![author_a => head_a]);

        // ...but an ack from the author's own sibling device does not.
        let newer_a = AckedOp {
            hash: Hash::digest(b"a6"),
            seq: 6,
        };
        db.record_message_ack(topic, sibling_a, author_a, newer_a)
            .await
            .unwrap();
        let delivered = db.delivered_acks(topic).await.unwrap();
        assert_eq!(delivered, maplit::btreemap![author_a => head_a]);

        // An acker with no agent mapping counts as another agent, and the
        // fold takes the max seq per author across ackers.
        let newest_a = AckedOp {
            hash: Hash::digest(b"a8"),
            seq: 8,
        };
        db.record_message_ack(topic, device(99), author_a, newest_a)
            .await
            .unwrap();
        db.record_message_ack(topic, device(99), author_b, head_b)
            .await
            .unwrap();
        let delivered = db.delivered_acks(topic).await.unwrap();
        assert_eq!(
            delivered,
            maplit::btreemap![author_a => newest_a, author_b => head_b]
        );
    }

    #[tokio::test]
    async fn test_tombstones_per_topic() {
        let dir = tempfile::tempdir().unwrap();
        let pool = create_sqlite_pool(dir.path().join("test_tombstones.db"))
            .await
            .unwrap();
        let db = OpProjection::new(pool.clone()).await.unwrap();

        let topic_a = *Topic::<kind::Untyped>::new([1; 32]);
        let topic_b = *Topic::<kind::Untyped>::new([2; 32]);
        let hash1 = Hash::digest(b"op1");
        let hash2 = Hash::digest(b"op2");

        assert!(!db.is_tombstoned(topic_a, hash1).await.unwrap());

        db.add_tombstone(topic_a, hash1, TombstoneReason::DeletedForEveryone)
            .await
            .unwrap();
        db.add_tombstone(topic_a, hash2, TombstoneReason::DeletedForMe)
            .await
            .unwrap();
        db.add_tombstone(topic_b, hash1, TombstoneReason::DeletedForEveryone)
            .await
            .unwrap();
        // Adding the same hash again is idempotent.
        db.add_tombstone(topic_a, hash1, TombstoneReason::DeletedForEveryone)
            .await
            .unwrap();

        assert!(db.is_tombstoned(topic_a, hash1).await.unwrap());
        assert!(db.is_tombstoned(topic_a, hash2).await.unwrap());
        // The reason is recorded per tombstone.
        assert_eq!(
            db.tombstone_reason(topic_a, hash1).await.unwrap(),
            Some(TombstoneReason::DeletedForEveryone)
        );
        assert_eq!(
            db.tombstone_reason(topic_a, hash2).await.unwrap(),
            Some(TombstoneReason::DeletedForMe)
        );
        // Tombstones are scoped per-topic: hash2 in topic_a does not leak into topic_b.
        assert!(db.is_tombstoned(topic_b, hash1).await.unwrap());
        assert!(!db.is_tombstoned(topic_b, hash2).await.unwrap());
        assert_eq!(db.tombstone_reason(topic_b, hash2).await.unwrap(), None);

        // Delete-for-me always wins: it upgrades an existing delete-for-everyone,
        // and a later delete-for-everyone never downgrades it back.
        db.add_tombstone(topic_a, hash1, TombstoneReason::DeletedForMe)
            .await
            .unwrap();
        assert_eq!(
            db.tombstone_reason(topic_a, hash1).await.unwrap(),
            Some(TombstoneReason::DeletedForMe)
        );
        db.add_tombstone(topic_a, hash1, TombstoneReason::DeletedForEveryone)
            .await
            .unwrap();
        assert_eq!(
            db.tombstone_reason(topic_a, hash1).await.unwrap(),
            Some(TombstoneReason::DeletedForMe)
        );

        assert_eq!(
            db.tombstoned_hashes(topic_a).await.unwrap(),
            maplit::btreeset![hash1, hash2]
        );
        assert_eq!(
            db.tombstoned_hashes(topic_b).await.unwrap(),
            maplit::btreeset![hash1]
        );

        // Tombstones persist across reopening the database.
        drop(db);
        pool.close().await;

        let pool = create_sqlite_pool(dir.path().join("test_tombstones.db"))
            .await
            .unwrap();
        let db = OpProjection::new(pool).await.unwrap();
        assert!(db.is_tombstoned(topic_a, hash1).await.unwrap());
        assert_eq!(
            db.tombstoned_hashes(topic_a).await.unwrap(),
            maplit::btreeset![hash1, hash2]
        );
    }
}
