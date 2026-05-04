pub(crate) mod author_operation;
mod stream_processing;

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::error::{AddContactError, Error, ShutdownError};
use crate::filesystem::Filesystem;
use anyhow::Result;
use chrono::{Duration, Utc};
use named_id::Rename;
use named_id::*;
use p2panda_auth::Access;
use p2panda_auth::group::resolver::StrongRemove;
use p2panda_auth::group::{GroupAction, GroupMember};
use p2panda_core::{Hash, PublicKey, Timestamp};
use p2panda_spaces::ActorId;
use p2panda_store::SqliteStore;
use tokio::sync::mpsc;

use mailbox_client::manager::{Mailboxes, MailboxesConfig};

use crate::chat::ChatMessageContent;
use crate::contact::{InboxTopic, QrCode, ShareIntent};
use crate::mailbox::MailboxOperation;
use crate::payload::{
    AnnouncementsPayload, ChatPayload, Extensions, InboxPayload, Payload, Profile,
};
use crate::stores::{GroupStore, LocalStore, NodeKeys, OpStore};
use crate::topic::{Topic, TopicId};
use crate::{
    AgentId, AsBody, ChatId, ChatReaction, DashAction, DeviceGroupId, DeviceGroupPayload, DeviceId,
    DirectChatId, Header, Operation,
};

pub use stream_processing::Notification;

#[derive(Clone, Debug)]
pub struct NodeConfig {
    pub contact_code_expiry: Duration,
    pub mailboxes_config: MailboxesConfig,
}

impl NodeConfig {
    #[cfg(feature = "testing")]
    pub fn testing() -> Self {
        let mut mailboxes_config = MailboxesConfig::default();
        mailboxes_config.active_interval = std::time::Duration::from_millis(1000);
        mailboxes_config.degraded_interval = std::time::Duration::from_millis(2000);
        mailboxes_config.stopped_interval = std::time::Duration::from_millis(5000);
        mailboxes_config.between_polls_delay = std::time::Duration::from_millis(100);
        Self {
            contact_code_expiry: Duration::days(7),
            mailboxes_config,
        }
    }
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            contact_code_expiry: Duration::days(7),
            mailboxes_config: MailboxesConfig::default(),
        }
    }
}

pub type DashResolver = StrongRemove<PublicKey, Hash, Operation, ()>;

#[derive(Clone)]
pub struct Node {
    pub op_store: OpStore,

    pub mailboxes: Mailboxes<MailboxOperation, OpStore>,

    // groups: p2panda_auth::group::Groups,
    config: NodeConfig,
    notification_tx: Option<mpsc::Sender<Notification>>,
    topic_subscribed_tx: Option<mpsc::Sender<TopicId>>,

    /// Add new subscription streams
    subscription_tx: mpsc::Sender<TopicId>,

    /// Abort trigger for the stream processing background task
    stream_cancel: Option<mpsc::Sender<()>>,
    /// Join handle for the stream processing background task
    stream_handle: Arc<Mutex<Option<std::thread::JoinHandle<()>>>>,

    local_store: LocalStore,
    group_store: GroupStore,
    node_keys: NodeKeys,

    filesystem: Filesystem,
}

impl Node {
    pub async fn new(
        data_path: PathBuf,
        config: NodeConfig,
        notification_tx: Option<mpsc::Sender<Notification>>,
        topic_subscribed_tx: Option<mpsc::Sender<TopicId>>,
    ) -> Result<Self> {
        let filesystem = Filesystem::new(data_path);
        let local_store = LocalStore::new(filesystem.local_store_path()).await?;
        let node_keys = local_store.node_keys().await?;

        Self::init(
            filesystem,
            local_store,
            node_keys,
            config,
            notification_tx,
            topic_subscribed_tx,
        )
        .await
    }

    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all, fields(me = ?node_keys.device_id().renamed())))]
    pub async fn init(
        filesystem: Filesystem,
        local_store: LocalStore,
        node_keys: NodeKeys,
        config: NodeConfig,
        notification_tx: Option<mpsc::Sender<Notification>>,
        topic_subscribed_tx: Option<mpsc::Sender<TopicId>>,
    ) -> Result<Self> {
        let op_store = OpStore::new(filesystem.op_store_path()).await?;
        let group_store = GroupStore::new(op_store.store.clone());

        let (subscription_tx, subscription_rx) = mpsc::channel(100);

        let mailboxes = Mailboxes::spawn(op_store.clone(), config.mailboxes_config.clone()).await?;

        let mut node = Self {
            op_store,
            mailboxes,
            config,
            filesystem,
            local_store: local_store.clone(),
            group_store,
            node_keys,
            notification_tx,
            subscription_tx,
            topic_subscribed_tx,
            stream_cancel: None,
            stream_handle: Arc::new(Mutex::new(None)),
        };

        let (cancel_tx, cancel_rx) = mpsc::channel(1);
        let handle = node.spawn_stream_process_loop(subscription_rx, cancel_rx);
        node.stream_cancel = Some(cancel_tx);
        node.stream_handle.lock().unwrap().replace(handle);

        node.initialize_stored_topics().await?;

        Ok(node)
    }

    pub fn data_path(&self) -> &PathBuf {
        self.filesystem.data_path()
    }

    pub async fn get_interleaved_logs(
        &self,
        topic_id: TopicId,
        authors: Vec<DeviceId>,
    ) -> anyhow::Result<Vec<(Header, Option<Payload>)>> {
        let mut logs = Vec::new();
        for author in authors {
            for op in self.op_store.get_log(&author, &topic_id, None).await? {
                if let Some(body) = op.body {
                    if let Ok(payload) = Payload::try_from_body(&body) {
                        logs.push((op.header, Some(payload)));
                    } else {
                        tracing::error!("Failed to decode payload: {body:?}");
                    }
                } else {
                    logs.push((op.header, None));
                }
            }
        }
        logs.sort_by_key(|(h, _)| h.timestamp);
        Ok(logs)
    }

    pub async fn get_authors(&self, topic_id: TopicId) -> anyhow::Result<HashSet<DeviceId>> {
        let authors = self
            .op_store
            .get_log_heights(&topic_id)
            .await?
            .keys()
            .cloned()
            .collect::<HashSet<_>>();
        Ok(authors)
    }

    pub async fn get_active_inbox_topics(&self) -> Result<BTreeSet<InboxTopic>, Error> {
        self.local_store
            .get_active_inbox_topics()
            .await
            .map_err(|err| Error::GetActiveInboxes(format!("{err}")))
    }

    /// Create a new contact QR code with configured expiry time,
    /// subscribe to the inbox topic for it, and register the topic as active.
    pub async fn new_qr_code(
        &self,
        share_intent: ShareIntent,
        inbox: bool,
    ) -> Result<QrCode, crate::Error> {
        let inbox_topic = if inbox {
            let inbox_topic = InboxTopic {
                topic: Topic::inbox().with_name(&format!("inbox({})", self.device_id().renamed())),
                expires_at: Utc::now() + self.config.contact_code_expiry,
            };
            self.initialize_topic(*inbox_topic.topic)
                .await
                .map_err(|err| crate::Error::InitializeTopic(format!("{err}")))?;
            self.local_store
                .add_active_inbox_topic(inbox_topic.clone())
                .await
                .map_err(|err| crate::Error::AddActiveInbox(format!("{err}")))?;
            Some(inbox_topic)
        } else {
            None
        };

        Ok(QrCode {
            device_pubkey: self.device_id(),
            inbox_topic,
            agent_id: self.node_keys.agent_id,
            share_intent,
        })
    }

    pub fn agent_id(&self) -> AgentId {
        self.node_keys.agent_id
    }

    pub fn device_id(&self) -> DeviceId {
        self.node_keys.device_id()
    }

    pub fn device_group_topic(&self) -> DeviceGroupId {
        Topic::device_group(self.agent_id()).into()
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
        self.register_topic(topic).await?;

        tracing::info!(
            my_actor = ?my_actor.renamed(),
            other = ?other.renamed(),
            topic = ?topic.renamed(),
            "creating direct chat space"
        );

        tracing::info!(?topic, ?topic, "created direct chat space");

        Ok(())
    }

    pub async fn create_group(
        &self,
        mut initial_members: BTreeMap<PublicKey, p2panda_auth::Access>,
    ) -> anyhow::Result<ChatId> {
        let chat_id = Topic::random();

        let device_ids: Vec<DeviceId> = initial_members
            .keys()
            .map(|public_key| DeviceId::from(*public_key))
            .collect();
        let contacts = self.local_store.lookup_contacts(&device_ids).await?;
        let agents: Vec<AgentId> = device_ids
            .iter()
            .filter_map(|did| match contacts.get(did) {
                Some(agent) => Some(*agent),
                None => {
                    tracing::warn!("Contact not found: {}", did.renamed());
                    None
                }
            })
            .collect();

        // The creator must always have Manage access
        initial_members.insert(*self.device_id(), p2panda_auth::Access::manage());

        let initial_members: Vec<_> = initial_members
            .into_iter()
            .map(|(public_key, access)| (GroupMember::Individual(public_key), access))
            .collect();

        let deps = self.group_store.heads().await?;
        self.author_operation(
            chat_id,
            DashAction::group_action(chat_id, GroupAction::Create { initial_members }, deps)?,
            Some(&format!("create_group({})", chat_id.renamed())),
        )
        .await?;

        self.register_topic(chat_id).await?;

        for agent in agents {
            self.invite_to_group(chat_id, agent).await?;
        }
        Ok(chat_id)
    }

    async fn invite_to_group(&self, chat_id: ChatId, person: AgentId) -> anyhow::Result<()> {
        let payload = Payload::Chat(ChatPayload::JoinGroup(chat_id));
        tracing::info!(
            "{} is inviting {} to group {}",
            self.device_id().renamed(),
            person.renamed(),
            chat_id.renamed(),
        );
        self.author_operation(
            self.direct_chat_topic(person),
            payload,
            Some(&format!(
                "invite_to_group({}, {})",
                chat_id.renamed(),
                person.renamed()
            )),
        )
        .await?;
        Ok(())
    }

    pub async fn add_group_member(
        &self,
        chat_id: ChatId,
        member: PublicKey,
        access: p2panda_auth::Access,
    ) -> anyhow::Result<()> {
        let deps = self.group_store.heads().await?;

        self.author_operation(
            chat_id,
            DashAction::group_action(
                chat_id,
                GroupAction::Add {
                    member: GroupMember::Individual(member),
                    access,
                },
                deps,
            )?,
            Some(&format!("add_group_member({})", chat_id.renamed())),
        )
        .await?;

        let agent_id = self
            .local_store
            .lookup_contact(DeviceId::from(member))
            .await?;
        if let Some(agent_id) = agent_id {
            self.invite_to_group(chat_id, agent_id).await?;
        } else {
            tracing::warn!("Contact not found: {}", DeviceId::from(member).renamed());
        }

        Ok(())
    }

    pub async fn remove_group_member(
        &self,
        chat_id: ChatId,
        member: PublicKey,
    ) -> anyhow::Result<()> {
        let deps = self.group_store.heads().await?;
        self.author_operation(
            chat_id,
            DashAction::group_action(
                chat_id,
                GroupAction::Remove {
                    member: GroupMember::Individual(member),
                },
                deps,
            )?,
            Some(&format!("remove_group_member({})", chat_id.renamed())),
        )
        .await?;
        Ok(())
    }

    pub async fn get_group_members(
        &self,
        chat_id: ChatId,
    ) -> anyhow::Result<BTreeSet<(DeviceId, Access)>> {
        let members = self
            .group_store
            .members(chat_id)
            .await?
            .into_iter()
            .map(|(m, a)| (DeviceId::from(m), a))
            .collect();
        Ok(members)
    }

    /// "Joining" a chat means subscribing to messages for that chat.
    /// This needs to be accompanied by being added as a member of the chat Space by an existing member
    /// -- you're not fully a member until someone adds you.
    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all, parent = None, fields(me = ?self.device_id().renamed())))]
    pub async fn join_group(&self, chat_id: ChatId) -> anyhow::Result<()> {
        tracing::info!(?chat_id, "joined group");
        self.register_topic(chat_id).await
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
        self.get_profile_for_agent(self.agent_id()).await
    }

    pub async fn lookup_contact(&self, device_id: DeviceId) -> anyhow::Result<Option<AgentId>> {
        self.local_store.lookup_contact(device_id).await
    }

    pub async fn all_contact_agent_ids(&self) -> anyhow::Result<Vec<AgentId>> {
        self.local_store.all_contact_agent_ids().await
    }

    pub async fn subscribed_topics(&self) -> anyhow::Result<std::collections::BTreeSet<TopicId>> {
        self.local_store.subscribed_topics().await
    }

    pub async fn get_profile_for_agent(
        &self,
        agent_id: AgentId,
    ) -> anyhow::Result<Option<Profile>> {
        let topic_id: TopicId = Topic::announcements(agent_id).into();
        let authors = self.get_authors(topic_id.clone()).await?;
        let ops = self
            .get_interleaved_logs(topic_id, authors.into_iter().collect())
            .await?;

        let mut set_profile_ops: Vec<(Timestamp, Profile)> = ops
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

    /// Abort the stream processing background task, allowing database handles to be released.
    pub async fn shutdown(&self) -> Result<(), ShutdownError> {
        // Stop polling mailboxes so the manager loop stops issuing OpStore queries.
        self.mailboxes.clear().await;

        if let Some(cancel) = self.stream_cancel.as_ref() {
            if let Err(err) = cancel.send(()).await {
                tracing::warn!(
                    "failed to send cancel signal to stream processing task: {}",
                    err
                );
                return Err(ShutdownError::StreamTaskJoin(Box::new(err)));
            }
        }
        tracing::info!("joining stream processing task");
        if let Some(handle) = self.stream_handle.lock().unwrap().take() {
            if let Err(err) = handle.join() {
                tracing::warn!("failed to join stream processing task: {:?}", err);
                return Err(ShutdownError::StreamTaskJoin(err));
            }
        }

        // Close pools last. SqlitePool clones share underlying state, so closing
        // here drains every connection across the app.
        self.local_store.close().await;
        self.op_store.close().await;

        Ok(())
    }

    /// Store someone as a contact, and:
    /// - register their spaces keybundle so we can add them to spaces
    /// - subscribe to their inbox
    /// - store them in the contacts map
    /// - send an invitation to them to do the same
    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all, fields(me = ?self.device_id().renamed())))]
    pub async fn add_contact(&self, contact: QrCode) -> Result<AgentId, AddContactError> {
        tracing::debug!("adding contact: {:?}", contact);

        self.local_store
            .save_contact(contact.clone())
            .await
            .map_err(|e| AddContactError::StoreContact(e.to_string()))?;

        // SPACES: Register the member in the spaces manager

        // Must subscribe to the new member's device group in order to receive their
        // group control messages.
        // TODO: is this idempotent? If not we must make sure to do this only once.
        self.register_topic(Topic::announcements(contact.agent_id))
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
        self.register_topic(direct_topic)
            .await
            .map_err(|e| Error::InitializeTopic(e.to_string()))?;

        self.author_operation(
            self.device_group_topic(),
            Payload::DeviceGroup(DeviceGroupPayload::AddContact(contact.clone())),
            Some(&format!("add_contact/add_contact({})", agent.renamed())),
        )
        .await
        .map_err(|e| Error::AuthorOperation(e.to_string()))?;

        if let Some(inbox_topic) = contact.inbox_topic.clone() {
            self.initialize_topic(*inbox_topic.topic)
                .await
                .map_err(|e| Error::InitializeTopic(e.to_string()))?;
            let code = self
                .new_qr_code(ShareIntent::AddContact, false)
                .await
                .map_err(|e| AddContactError::CreateQrCode(e.to_string()))?;
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
                Some(&format!("add_contact/contact_request({})", agent.renamed())),
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

    /// Mark messages as read by storing a ReadMessages operation in the device group topic.
    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all, fields(me = ?self.device_id().renamed())))]
    pub async fn mark_messages_read(
        &self,
        chat_id: ChatId,
        message_hashes: Vec<p2panda_core::Hash>,
    ) -> Result<(), Error> {
        use crate::payload::ReadMessagesPayload;

        self.author_operation(
            self.device_group_topic(),
            Payload::DeviceGroup(DeviceGroupPayload::ReadMessages(ReadMessagesPayload {
                chat_id,
                message_hashes,
            })),
            Some(&format!("mark_messages_read({})", chat_id.renamed())),
        )
        .await
        .map_err(|e| Error::AuthorOperation(e.to_string()))?;

        Ok(())
    }

    async fn initialize_stored_topics(&self) -> anyhow::Result<()> {
        self.initialize_topic(
            *Topic::announcements(self.agent_id())
                .with_name(&format!("announce({})", self.agent_id().renamed())),
        )
        .await?;

        for topic in self.local_store.get_active_inbox_topics().await?.iter() {
            self.initialize_topic(
                *topic
                    .topic
                    .clone()
                    .with_name(&format!("inbox({})", self.device_id().renamed())),
            )
            .await?;
        }

        for topic in self.local_store.subscribed_topics().await?.iter() {
            self.initialize_topic(*topic).await?;
        }

        Ok(())
    }
}
