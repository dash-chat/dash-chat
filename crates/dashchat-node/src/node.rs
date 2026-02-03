pub(crate) mod author_operation;
mod stream_processing;

use std::collections::{BTreeSet, HashSet};
use std::path::PathBuf;
use std::pin::Pin;

use anyhow::Result;

use crate::error::{AddContactError, Error};
use crate::filesystem::Filesystem;
use chrono::{Duration, Utc};
use futures::Stream;
use named_id::Rename;
use named_id::*;
use p2panda_core::Body;
use p2panda_net::ResyncConfiguration;
use p2panda_spaces::ActorId;
use p2panda_store::{LogStore, SqliteStore};
use p2panda_stream::IngestExt;
use p2panda_stream::partial::operations::PartialOrder;
use tokio::sync::mpsc;

use mailbox_client::manager::{Mailboxes, MailboxesConfig};

use crate::chat::ChatMessageContent;
use crate::contact::{ContactCode, InboxTopic, ShareIntent};
use crate::local_store::NodeData;
use crate::mailbox::MailboxOperation;
use crate::payload::{
    AnnouncementsPayload, ChatPayload, Extensions, InboxPayload, Payload, Profile,
};
use crate::stores::OpStore;
use crate::topic::{Topic, TopicId};
use crate::{
    AgentId, AsBody, ChatId, ChatReaction, DeviceGroupId, DeviceGroupPayload, DeviceId,
    DirectChatId, Header, Operation,
};

pub use crate::local_store::LocalStore;
pub use stream_processing::Notification;

#[derive(Clone, Debug)]
pub struct NodeConfig {
    pub resync: ResyncConfiguration,
    pub contact_code_expiry: Duration,
    pub mailboxes_config: MailboxesConfig,
}

impl NodeConfig {
    #[cfg(feature = "testing")]
    pub fn testing() -> Self {
        let mut mailboxes_config = MailboxesConfig::default();
        mailboxes_config.success_interval = std::time::Duration::from_millis(1000);
        mailboxes_config.error_interval = std::time::Duration::from_millis(1000);
        Self {
            resync: ResyncConfiguration::new().interval(3).poll_interval(1),
            contact_code_expiry: Duration::days(7),
            mailboxes_config,
        }
    }
}

impl Default for NodeConfig {
    fn default() -> Self {
        let resync = ResyncConfiguration::new().interval(3).poll_interval(1);
        Self {
            resync,
            contact_code_expiry: Duration::days(7),
            mailboxes_config: MailboxesConfig::default(),
        }
    }
}

pub type Orderer<S> =
    PartialOrder<TopicId, Extensions, S, p2panda_stream::partial::MemoryStore<p2panda_core::Hash>>;

pub type NodeOpStore = OpStore<SqliteStore<TopicId, Extensions>>;

#[derive(Clone)]
pub struct Node {
    pub op_store: NodeOpStore,

    pub mailboxes: Mailboxes<MailboxOperation, NodeOpStore>,

    // groups: p2panda_auth::group::Groups,
    config: NodeConfig,
    notification_tx: Option<mpsc::Sender<Notification>>,

    /// Add new subscription streams
    stream_tx: mpsc::Sender<Pin<Box<dyn Stream<Item = Operation> + Send + 'static>>>,

    filesystem: Filesystem,
    local_store: LocalStore,
    node_data: NodeData,
}

impl Node {
    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
    pub async fn new(
        data_path: PathBuf,
        config: NodeConfig,
        notification_tx: Option<mpsc::Sender<Notification>>,
    ) -> Result<Self> {
        let filesystem = Filesystem::new(data_path);
        let local_store = LocalStore::new(filesystem.local_store_path())?;
        let node_data = local_store.node_data()?;

        // let op_store = OpStore::new_memory();
        let op_store = OpStore::new_sqlite(filesystem.op_store_path()).await?;

        let (stream_tx, stream_rx) = mpsc::channel(100);

        let mailboxes = Mailboxes::spawn(op_store.clone(), config.mailboxes_config.clone()).await?;

        let node = Self {
            op_store: op_store.clone(),
            mailboxes,
            config,
            filesystem,
            local_store: local_store.clone(),
            node_data,
            notification_tx,
            stream_tx,
        };

        node.spawn_stream_process_loop(stream_rx);

        node.initialize_topic(
            Topic::announcements(node.agent_id())
                .with_name(&format!("announce({})", node.agent_id().renamed())),
            true,
        )
        .await?;

        for topic in local_store.get_active_inbox_topics()?.iter() {
            node.initialize_topic(
                topic
                    .topic
                    .clone()
                    .with_name(&format!("inbox({})", node.device_id().renamed())),
                false,
            )
            .await?;
        }

        // TODO: locally store list of groups and initialize them when the node starts

        Ok(node)
    }

    pub async fn get_interleaved_logs(
        &self,
        topic_id: TopicId,
        authors: Vec<DeviceId>,
    ) -> anyhow::Result<Vec<(Header, Option<Payload>)>> {
        let mut logs = Vec::new();
        for author in authors {
            for (h, b) in self.get_log(topic_id, author).await? {
                if let Some(body) = b {
                    if let Ok(payload) = Payload::try_from_body(&body) {
                        logs.push((h, Some(payload)));
                    } else {
                        tracing::error!("Failed to decode payload: {body:?}");
                    }
                } else {
                    logs.push((h, None));
                }
            }
        }
        logs.sort_by_key(|(h, _)| h.timestamp);
        Ok(logs)
    }

    pub async fn get_log(
        &self,
        topic: TopicId,
        author: DeviceId,
    ) -> anyhow::Result<Vec<(Header, Option<Body>)>> {
        let _heights = self.op_store.get_log_heights(&topic).await?;
        match self.op_store.get_log(&author, &topic, None).await? {
            Some(log) => Ok(log),
            None => {
                let author = *author;
                tracing::warn!("No log found for topic {topic:?} and author {author:?}");
                Ok(vec![])
            }
        }
    }

    pub async fn get_authors(&self, topic_id: TopicId) -> anyhow::Result<HashSet<DeviceId>> {
        let authors = self
            .op_store
            .get_log_heights(&topic_id)
            .await?
            .into_iter()
            .map(|(pk, _)| DeviceId::from(pk))
            .collect::<HashSet<_>>();
        Ok(authors)
    }

    pub fn get_active_inbox_topics(&self) -> Result<BTreeSet<InboxTopic>, Error> {
        self.local_store
            .get_active_inbox_topics()
            .map_err(|err| Error::GetActiveInboxes(format!("{err}")))
    }

    /// Create a new contact QR code with configured expiry time,
    /// subscribe to the inbox topic for it, and register the topic as active.
    pub async fn new_qr_code(
        &self,
        share_intent: ShareIntent,
        inbox: bool,
    ) -> Result<ContactCode, crate::Error> {
        let inbox_topic = if inbox {
            let inbox_topic = InboxTopic {
                topic: Topic::inbox().with_name(&format!("inbox({})", self.device_id().renamed())),
                expires_at: Utc::now() + self.config.contact_code_expiry,
            };
            self.initialize_topic(inbox_topic.topic, false)
                .await
                .map_err(|err| crate::Error::InitializeTopic(format!("{err}")))?;
            self.local_store
                .add_active_inbox_topic(inbox_topic.clone())
                .map_err(|err| crate::Error::AddActiveInbox(format!("{err}")))?;
            Some(inbox_topic)
        } else {
            None
        };

        Ok(ContactCode {
            device_pubkey: self.device_id(),
            inbox_topic,
            agent_id: self.node_data.agent_id,
            share_intent,
        })
    }

    /// Get the stored contact code if it exists and is not expired,
    /// otherwise create a new one and store it.
    pub async fn get_or_create_contact_code(&self) -> Result<ContactCode, crate::ContactCodeError> {
        // Check if we have a stored contact code
        if let Some(stored_code) = self
            .local_store
            .get_contact_code()
            .map_err(|e| crate::ContactCodeError::GetContactCode(format!("{e}")))?
        {
            // Check if the inbox topic is still valid (not expired)
            if let Some(inbox_topic) = &stored_code.inbox_topic {
                if inbox_topic.expires_at > Utc::now() {
                    return Ok(stored_code);
                }
                // Expired - remove from active inboxes and create new
                if let Err(err) = self
                    .local_store
                    .remove_active_inbox_topic(&inbox_topic.topic)
                {
                    tracing::error!("Failed to remove expired inbox topic: {}", err);
                }
            } else {
                // No inbox topic means this is a response code, should still be valid
                return Ok(stored_code);
            }
        }
        // Create a new contact code and store it
        let new_code = self.new_qr_code(ShareIntent::AddContact, true).await?;
        self.local_store
            .set_contact_code(&new_code)
            .map_err(|e| crate::ContactCodeError::SetContactCode(format!("{e}")))?;
        Ok(new_code)
    }

    /// Reset the contact code: remove the old inbox topic from active inboxes,
    /// clear the stored code, and create a new one.
    pub async fn reset_contact_code(&self) -> Result<ContactCode, crate::ContactCodeError> {
        // Get the current stored code to clean up its inbox topic
        if let Ok(Some(stored_code)) = self.local_store.get_contact_code() {
            if let Some(inbox_topic) = &stored_code.inbox_topic {
                // Remove from active inboxes so we stop listening for messages on this topic
                let _ = self
                    .local_store
                    .remove_active_inbox_topic(&inbox_topic.topic);
            }
        }

        // Clear the stored code
        self.local_store
            .clear_contact_code()
            .map_err(|e| crate::ContactCodeError::ClearContactCode(format!("{e}")))?;

        // Create a new contact code and store it
        let new_code = self.new_qr_code(ShareIntent::AddContact, true).await?;
        self.local_store
            .set_contact_code(&new_code)
            .map_err(|e| crate::ContactCodeError::SetContactCode(format!("{e}")))?;
        Ok(new_code)
    }

    pub fn agent_id(&self) -> AgentId {
        self.node_data.agent_id
    }

    /// Get the topic for a direct chat between two public keys.
    ///
    /// The topic is the hashed sorted public keys.
    /// Anyone who knows the two public keys can derive the same topic.
    // TODO: is this a problem? Should we use a random topic instead?
    pub fn direct_chat_topic(&self, other: AgentId) -> DirectChatId {
        let me = self.agent_id();
        // TODO: use two secrets from each party to construct the topic
        let topic = Topic::direct_chat([me, other]);
        if me > other {
            topic.with_name(&format!("direct({},{})", other.renamed(), me.renamed()))
        } else {
            topic.with_name(&format!("direct({},{})", me.renamed(), other.renamed()))
        }
    }

    /// Create a new direct chat Space.
    /// Note that only one node should create the space!
    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all, fields(me = ?self.device_id().renamed())))]
    pub async fn create_direct_chat_space(&self, other: AgentId) -> anyhow::Result<()> {
        let topic = self.direct_chat_topic(other);

        let my_actor = self.agent_id();
        self.initialize_topic(topic, true).await?;

        tracing::info!(
            my_actor = ?my_actor.renamed(),
            other = ?other.renamed(),
            topic = ?topic.renamed(),
            "creating direct chat space"
        );

        tracing::info!(?topic, ?topic, "created direct chat space");

        Ok(())
    }

    /// "Joining" a chat means subscribing to messages for that chat.
    /// This needs to be accompanied by being added as a member of the chat Space by an existing member
    /// -- you're not fully a member until someone adds you.
    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all, parent = None, fields(me = ?self.device_id().renamed())))]
    pub async fn join_group(&self, chat_id: ChatId) -> anyhow::Result<()> {
        tracing::info!(?chat_id, "joined group");
        self.initialize_topic(chat_id, true).await
    }

    pub async fn set_profile(&self, profile: Profile) -> Result<(), crate::Error> {
        self.author_operation(
            Topic::announcements(self.agent_id()),
            Payload::Announcements(AnnouncementsPayload::SetProfile(profile)),
            Some(&format!("set_profile({})", self.device_id().renamed())),
        )
        .await
        .map_err(|e| Error::AuthorOperation(e.to_string()))?;

        Ok(())
    }

    pub async fn my_profile(&self) -> anyhow::Result<Option<Profile>> {
        let topic_id: TopicId = Topic::announcements(self.agent_id()).into();
        let authors = self.get_authors(topic_id.clone()).await?;
        let ops = self
            .get_interleaved_logs(topic_id, authors.into_iter().collect())
            .await?;

        let mut set_profile_ops: Vec<(u64, Profile)> = ops
            .into_iter()
            .filter_map(|(header, payload)| match payload {
                Some(Payload::Announcements(AnnouncementsPayload::SetProfile(profile))) => {
                    Some((header.timestamp, profile))
                }
                _ => None,
            })
            .collect();

        set_profile_ops.sort_by_key(|(timestamp, _)| *timestamp);

        let Some((_, profile)) = set_profile_ops.last() else {
            return Ok(None);
        };
        Ok(Some(profile.clone()))
    }

    /// Get a reference to the local store for testing purposes.
    #[cfg(feature = "testing")]
    pub fn local_store(&self) -> &LocalStore {
        &self.local_store
    }

    /// Get all messages for a chat from the logs.
    ///
    /// In the real app, the interleaving of logs happens on the front end.
    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all, fields(me = ?self.device_id().renamed())))]
    #[cfg(feature = "testing")]
    pub async fn get_messages(
        &self,
        topic: impl Into<ChatId>,
    ) -> anyhow::Result<Vec<crate::chat::testing::ChatMessage>> {
        let chat_id = topic.into();
        let mut messages = vec![];

        let authors = self.get_authors(chat_id.into()).await?;

        for (header, payload) in self
            .get_interleaved_logs(chat_id.into(), authors.into_iter().collect())
            .await?
        {
            if let Some(Payload::Chat(ChatPayload::Message(message))) = payload {
                messages.push(crate::chat::testing::ChatMessage::new(message, &header));
            }
        }

        // for (events, author, timestamp) in events {
        //     for event in events {
        //         use crate::Cbor;
        //         match event {
        //             Event::Application { space_id, data } => {
        //                 messages.push(ChatMessage::from_bytes(&data)?)
        //             }
        //             _ => {}
        //         }
        //     }
        // }

        Ok(messages)
    }

    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all, fields(me = ?self.device_id().renamed())))]
    pub async fn send_message(
        &self,
        topic: impl Into<ChatId>,
        message: ChatMessageContent,
    ) -> anyhow::Result<Header> {
        let topic = topic.into();

        let message = ChatMessageContent::from(message);

        let header = self
            .author_operation(
                topic,
                Payload::Chat(ChatPayload::Message(message.clone())),
                None,
            )
            .await?;

        Ok(header)
    }

    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all, fields(me = ?self.device_id().renamed())))]
    pub async fn add_reaction(
        &self,
        topic: impl Into<ChatId>,
        reaction: ChatReaction,
    ) -> anyhow::Result<Header> {
        let topic = topic.into();
        let header = self
            .author_operation(topic, Payload::Chat(ChatPayload::Reaction(reaction)), None)
            .await?;

        Ok(header)
    }

    pub fn device_id(&self) -> DeviceId {
        self.node_data.device_id()
    }

    pub fn device_group_topic(&self) -> DeviceGroupId {
        Topic::device_group(self.agent_id()).into()
    }

    /// Store someone as a contact, and:
    /// - register their spaces keybundle so we can add them to spaces
    /// - subscribe to their inbox
    /// - store them in the contacts map
    /// - send an invitation to them to do the same
    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all, fields(me = ?self.device_id().renamed())))]
    pub async fn add_contact(&self, contact: ContactCode) -> Result<AgentId, AddContactError> {
        tracing::debug!("adding contact: {:?}", contact);

        // SPACES: Register the member in the spaces manager

        // Must subscribe to the new member's device group in order to receive their
        // group control messages.
        // TODO: is this idempotent? If not we must make sure to do this only once.
        self.initialize_topic(Topic::announcements(contact.agent_id), false)
            .await
            .map_err(|e| Error::InitializeTopic(e.to_string()))?;

        // TODO: use all of this commented out stuff when spaces are possible again
        // // XXX: there should be a better way to wait for the device group to be created,
        // //      and this may never happen if the contact is not online.
        // let mut attempts = 0;
        // loop {
        //     if let Some(group) = self.manager.group(contact.chat_actor_id).await? {
        //         if group
        //             .members()
        //             .await?
        //             .iter()
        //             .map(|(id, _)| *id)
        //             .any(|id| id == member_id)
        //         {
        //             break;
        //         }
        //     }

        //     // // see https://github.com/p2panda/p2panda/pull/871
        //     // if let Some(space) = self.manager.space(contact.device_space_id.into()).await? {
        //     //     if space
        //     //         .members()
        //     //         .await?
        //     //         .iter()
        //     //         .map(|(id, _)| *id)
        //     //         .any(|id| id == member_id)
        //     //     {
        //     //         break;
        //     //     }
        //     // }

        //     tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        //     attempts += 1;
        //     if attempts > 20 {
        //         return Err(anyhow!(
        //             "Failed to register contact's device group in 5s. Try again later."
        //         ));
        //     }
        // }
        // // XXX: need sleep a little more for all the messages to be processed
        // tokio::time::sleep(std::time::Duration::from_millis(3000)).await;

        // self.initialize_topic(Topic::announcements(actor), false)
        //     .await?;

        let agent = contact.agent_id;
        let direct_topic = self.direct_chat_topic(agent);
        self.initialize_topic(direct_topic, true)
            .await
            .map_err(|e| Error::InitializeTopic(e.to_string()))?;

        self.author_operation(
            self.device_group_topic(),
            Payload::DeviceGroup(DeviceGroupPayload::AddContact(contact.clone())),
            Some(&format!("add_contact/invitation({})", agent.renamed())),
        )
        .await
        .map_err(|e| Error::AuthorOperation(e.to_string()))?;

        if let Some(inbox_topic) = contact.inbox_topic.clone() {
            self.initialize_topic(inbox_topic.topic, true)
                .await
                .map_err(|e| Error::InitializeTopic(e.to_string()))?;
            let code = self
                .new_qr_code(ShareIntent::AddContact, false)
                .await
                .map_err(|e| AddContactError::CreateContactCode(e.to_string()))?;
            let Some(profile) = self
                .my_profile()
                .await
                .map_err(|e| Error::AuthorOperation(e.to_string()))?
            else {
                return Err(AddContactError::ProfileNotCreated);
            };
            self.author_operation(
                inbox_topic.topic,
                Payload::Inbox(InboxPayload::ContactRequest { code, profile }),
                Some(&format!("add_contact/invitation({})", agent.renamed())),
            )
            .await
            .map_err(|e| Error::AuthorOperation(e.to_string()))?;
        }

        // Only the initiator of contactship should create the direct chat space
        if contact.share_intent == ShareIntent::AddContact && contact.inbox_topic.is_none() {
            self.create_direct_chat_space(agent)
                .await
                .map_err(|e| AddContactError::CreateDirectChat(e.to_string()))?;
        }

        Ok(agent)
    }

    /// Reject a contact request from the given agent.
    /// This creates a RejectContactRequest operation in the device group topic.
    /// Contact requests made before this rejection will be filtered out.
    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all, fields(me = ?self.device_id().renamed())))]
    pub async fn reject_contact_request(&self, agent_id: AgentId) -> Result<(), Error> {
        tracing::debug!("rejecting contact request from: {:?}", agent_id);

        self.author_operation(
            self.device_group_topic(),
            Payload::DeviceGroup(DeviceGroupPayload::RejectContactRequest(agent_id)),
            Some(&format!("reject_contact_request({})", agent_id.renamed())),
        )
        .await
        .map_err(|e| Error::AuthorOperation(e.to_string()))?;

        Ok(())
    }

    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all, fields(me = ?self.device_id().renamed())))]
    pub async fn remove_contact(&self, _chat_actor_id: ActorId) -> anyhow::Result<()> {
        // TODO: shutdown inbox task, etc.
        todo!("add tombstone to contacts list");
    }
}
