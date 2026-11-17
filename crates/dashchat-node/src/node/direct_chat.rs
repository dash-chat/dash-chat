//! A direct chat both sides requested at once.
//!
//! Each requester created a space, so the chat is spread over two until the
//! side whose space lost the tie has moved its messages into the other one.

use std::collections::{BTreeSet, HashMap};

use super::*;
use crate::chat::MovedMessage;

/// What a running node keeps track of across its direct chats.
#[derive(Clone, Default)]
pub(crate) struct DirectChats {
    admissions: Arc<Mutex<Vec<JoinHandle<()>>>>,
    /// Held while admitting, so two admissions of the same peer don't both add them.
    admitting: Arc<Mutex<()>>,
    /// Held while moving messages between spaces, so none is moved twice.
    moving: Arc<Mutex<()>>,
}

impl DirectChats {
    /// Have `node` admit `peer` to the direct chat space it created for them,
    /// in the background: the key bundle of someone whose code we scanned
    /// only arrives once we sync their inbox.
    pub(crate) async fn spawn_admission(&self, node: Node, peer: DeviceId, chat_id: DirectChatId) {
        let handle = tokio::spawn(async move {
            if let Err(err) = node.admit_to_direct_chat(peer, chat_id).await {
                tracing::warn!(?err, peer = ?peer.aliased(), "failed to admit to direct chat");
            }
        });
        self.admissions.lock().await.push(handle);
    }

    pub(crate) async fn abort_admissions(&self) {
        for handle in self.admissions.lock().await.drain(..) {
            handle.abort();
        }
    }
}

/// The two spaces of a direct chat both sides requested, and who gives theirs up.
struct SplitDirectChat {
    winning: DirectChatId,
    losing: DirectChatId,
    loser: DeviceId,
}

impl Node {
    /// The space our direct chat with `peer` takes place in, once either of us
    /// has requested contact.
    ///
    /// When both did, each created a space. Until the two get reconciled, both
    /// sides settle on the one requested by the lower device id.
    pub async fn direct_chat_id(&self, peer: DeviceId) -> anyhow::Result<Option<DirectChatId>> {
        let spaces = self.projection.direct_chat_spaces(peer).await?;
        let preferred_requester = self.device_id().min(peer);
        Ok(spaces
            .iter()
            .find(|(_, requester)| *requester == preferred_requester)
            .or(spaces.first())
            .map(|(space, _)| *space))
    }

    #[cfg(feature = "testing")]
    pub async fn direct_chat_with(&self, other: &TestNode) -> DirectChatId {
        self.direct_chat_id(other.device_id())
            .await
            .unwrap()
            .expect("no contact request was exchanged with this node")
    }

    /// [`Self::direct_chat_id`] of someone we exchanged a contact request with.
    pub(crate) async fn known_direct_chat_id(
        &self,
        peer: DeviceId,
    ) -> anyhow::Result<DirectChatId> {
        self.direct_chat_id(peer)
            .await?
            .ok_or_else(|| anyhow::anyhow!("no direct chat with {:?}", peer.aliased()))
    }

    /// The space we created for our direct chat with `peer`, if we requested
    /// contact with them before.
    pub(crate) async fn requested_direct_chat_id(
        &self,
        peer: DeviceId,
    ) -> anyhow::Result<Option<DirectChatId>> {
        let spaces = self.projection.direct_chat_spaces(peer).await?;
        Ok(spaces
            .into_iter()
            .find(|(_, requester)| *requester == self.device_id())
            .map(|(space, _)| space))
    }

    /// Create the space for a direct chat we are requesting, with only us in
    /// it: the other side is admitted once their key bundle has reached us.
    pub(super) async fn create_direct_chat_space(&self) -> anyhow::Result<DirectChatId> {
        let (chat_id, _) = self.create_space(vec![]).await?;
        self.publish_my_profile_to(chat_id).await?;
        tracing::info!(chat_id = ?chat_id.aliased(), "created direct chat space");
        Ok(chat_id)
    }

    /// Admit `peer` to the direct chat space we created for them, once their
    /// key bundle has reached us.
    pub(crate) async fn admit_to_direct_chat(
        &self,
        peer: DeviceId,
        chat_id: DirectChatId,
    ) -> anyhow::Result<()> {
        self.keybundle_of(peer).await?;
        let _admitting = self.direct_chats.admitting.lock().await;
        let is_member = self
            .space_members(chat_id.into())
            .await?
            .iter()
            .any(|(member, _)| *member == peer);
        if is_member {
            return Ok(());
        }
        let space = self.streams.space(chat_id.into()).await?;
        self.add_member_to_space(&space, *peer, AccessLevel::Write)
            .await?;
        Ok(())
    }

    /// Every space the chat in `chat_id` is spread over: both spaces of a
    /// direct chat each side requested, otherwise just the chat's own.
    pub async fn chat_spaces(&self, chat_id: ChatId) -> anyhow::Result<Vec<ChatId>> {
        let Some(peer) = self.projection.direct_chat_peer(chat_id).await? else {
            return Ok(vec![chat_id]);
        };
        let spaces = self.projection.direct_chat_spaces(peer).await?;
        Ok(spaces.into_iter().map(|(space, _)| space).collect())
    }

    /// The space to publish into for the chat in `chat_id`. Once we know our
    /// space of a direct chat lost the tie, that is the other side's, and our
    /// earlier messages are moved there before anything new is written.
    pub(crate) async fn chat_write_target(&self, chat_id: ChatId) -> anyhow::Result<ChatId> {
        let Some(peer) = self.projection.direct_chat_peer(chat_id).await? else {
            return Ok(chat_id);
        };
        let Some(split) = self.split_direct_chat(peer).await? else {
            return Ok(self.direct_chat_id(peer).await?.unwrap_or(chat_id));
        };
        if split.loser == self.device_id() {
            // Boxed: this rarely-taken path would otherwise bloat the future
            // of every publish, which p2panda's deep futures leave no room for.
            Box::pin(self.move_direct_chat_messages(&split)).await?;
        }
        Ok(split.winning)
    }

    async fn split_direct_chat(&self, peer: DeviceId) -> anyhow::Result<Option<SplitDirectChat>> {
        let spaces = self.projection.direct_chat_spaces(peer).await?;
        let Some(winning) = self.direct_chat_id(peer).await? else {
            return Ok(None);
        };
        Ok(spaces
            .into_iter()
            .find(|(space, _)| *space != winning)
            .map(|(losing, loser)| SplitDirectChat {
                winning,
                losing,
                loser,
            }))
    }

    /// Both of us requested contact, so the other's request is their answer
    /// to ours: no acceptance is sent either way. Whoever's space lost the tie
    /// moves over to the other's.
    pub(crate) async fn complete_crossed_request(
        &self,
        peer: DeviceId,
        agent_id: AgentId,
    ) -> anyhow::Result<()> {
        if self.is_accepted_contact(peer).await? {
            return Ok(());
        }
        // Unlike an acceptance, their request does not show ours reached them,
        // so we keep serving the inbox it is on.
        self.register_bootstrap_node(*peer).await?;
        let direct_chat_id = self.known_direct_chat_id(peer).await?;
        self.join_space(direct_chat_id).await?;
        let direct_chat_id = self.chat_write_target(direct_chat_id).await?;
        self.publish_add_contact(agent_id, direct_chat_id).await?;
        if let Err(err) = self.publish_my_profile_to(direct_chat_id).await {
            tracing::warn!(?err, "failed to push my profile to the direct chat");
        }
        self.mark_ack_topic_dirty(direct_chat_id);
        self.drop_moved_direct_chat_space(peer).await
    }

    /// Copy the messages we wrote in our own space of a direct chat into the
    /// other side's, oldest first. Safe to repeat: a message already moved is
    /// not moved again.
    async fn move_direct_chat_messages(&self, split: &SplitDirectChat) -> anyhow::Result<()> {
        let _moving = self.direct_chats.moving.lock().await;
        let me = self.device_id();
        let mut moved = self.moved_messages(me, split.winning).await?;

        let mut pending = vec![];
        let mut edits: HashMap<Hash, (Hash, String)> = HashMap::new();
        let mut deleted: BTreeSet<Hash> = BTreeSet::new();
        for op in self
            .op_store
            .get_log(&me, &LogId::from(split.losing), None)
            .await?
        {
            let Ok(Some(body)) = &op.body else {
                continue;
            };
            match Payload::try_from_body(body) {
                Ok(Payload::Chat(ChatPayload::Message(message))) => {
                    pending.push((op.header.clone(), message))
                }
                Ok(Payload::Chat(ChatPayload::EditMessage { message, edit_hash })) => {
                    edits.insert(edit_hash, (op.header.hash(), message));
                }
                Ok(Payload::Chat(ChatPayload::DeleteMessage { hashes })) => deleted.extend(hashes),
                _ => {}
            }
        }

        for (header, mut message) in pending {
            let original = header.hash();
            // Replies name the latest edit of their target, so every hash of
            // this message's edit chain leads to its copy.
            let mut chain = vec![original];
            while let Some((edit, text)) = edits.get(chain.last().expect("never empty")) {
                message.message = text.clone();
                chain.push(*edit);
            }
            if deleted.contains(&original) {
                continue;
            }
            let copy = match moved.get(&original) {
                Some(copy) => *copy,
                None => {
                    message.reply = message
                        .reply
                        .map(|target| moved.get(&target).copied().unwrap_or(target));
                    message.moved = Some(MovedMessage {
                        hash: original,
                        timestamp: header.extensions.timestamp().into(),
                    });
                    self.publish_moved_message(split.winning, message).await?
                }
            };
            for hash in chain {
                moved.insert(hash, copy);
            }
        }
        Ok(())
    }

    /// Publish our copy of a message into the space it moves to, with its
    /// media encrypted anew towards that space.
    async fn publish_moved_message(
        &self,
        space: DirectChatId,
        mut message: ChatMessageContent,
    ) -> anyhow::Result<Hash> {
        let topic: TopicId = space.into();
        if let Some(media) = message.media.take() {
            let media = self.load_media(media).await?;
            message.media = Some(self.store_media(topic, SENTINEL_OP_HASH, media).await?);
        }
        let media = message.media.clone();
        let header = self
            .publish_unguarded(topic, Payload::Chat(ChatPayload::Message(message)))
            .await?;
        for item in media.iter().flatten() {
            if let Err(err) = self
                .require_blob_sync()?
                .retag_blob(topic, self.device_id(), header.hash(), item.hash())
                .await
            {
                tracing::warn!(?err, "failed to retag blob after operation creation");
            }
        }
        Ok(header.hash())
    }

    /// The messages `author` moved into `space`: each original's hash with
    /// the hash of its copy.
    async fn moved_messages(
        &self,
        author: DeviceId,
        space: DirectChatId,
    ) -> anyhow::Result<HashMap<Hash, Hash>> {
        let mut moved = HashMap::new();
        for op in self
            .op_store
            .get_log(&author, &LogId::from(space), None)
            .await?
        {
            let Ok(Some(body)) = &op.body else {
                continue;
            };
            let Ok(Payload::Chat(ChatPayload::Message(message))) = Payload::try_from_body(body)
            else {
                continue;
            };
            if let Some(origin) = message.moved() {
                moved.insert(origin.hash, op.header.hash());
            }
        }
        Ok(moved)
    }

    /// Give up the losing space of a direct chat both sides requested, once
    /// nothing we hold of it is missing from the winning one. Until then both
    /// stay, so no message disappears on its way over.
    pub(crate) async fn drop_moved_direct_chat_space(&self, peer: DeviceId) -> anyhow::Result<()> {
        let Some(split) = self.split_direct_chat(peer).await? else {
            return Ok(());
        };
        if !self.is_accepted_contact(peer).await? {
            return Ok(());
        }
        let moved = self.moved_messages(split.loser, split.winning).await?;
        // The loser keeps writing to its own space until our request reaches it.
        let loser_log_in_winning = self
            .op_store
            .get_log(&split.loser, &LogId::from(split.winning), None)
            .await?;
        if split.loser != self.device_id() && loser_log_in_winning.is_empty() {
            return Ok(());
        }
        for op in self
            .op_store
            .get_log(&split.loser, &LogId::from(split.losing), None)
            .await?
        {
            let Ok(Some(body)) = &op.body else {
                continue;
            };
            let is_message = matches!(
                Payload::try_from_body(body),
                Ok(Payload::Chat(ChatPayload::Message(_)))
            );
            if is_message && !moved.contains_key(&op.header.hash()) {
                return Ok(());
            }
        }

        tracing::info!(losing = ?split.losing.aliased(), "dropping the direct chat space that lost the tie");
        self.projection.drop_direct_chat_space(split.losing).await?;
        self.local_store
            .register_topic_as_unsubscribed(split.losing)
            .await?;
        self.online_peers.leave(*split.losing).await;
        self.mailboxes.unsubscribe(split.losing.into()).await?;
        self.streams.unsubscribe(*split.losing).await;
        Ok(())
    }

    /// Carry on with any direct chat still spread over two spaces, e.g. after
    /// a restart partway through moving its messages.
    pub(crate) async fn resume_split_direct_chats(&self) -> anyhow::Result<()> {
        for peer in self.projection.split_direct_chat_peers().await? {
            if !self.is_accepted_contact(peer).await? {
                continue;
            }
            let direct_chat_id = self.known_direct_chat_id(peer).await?;
            self.chat_write_target(direct_chat_id).await?;
            self.drop_moved_direct_chat_space(peer).await?;
        }
        Ok(())
    }
}
