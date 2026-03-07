pub(crate) mod author_operation;
mod stream_processing;

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;

use anyhow::Result;
use p2panda_auth::Access;
use p2panda_auth::group::resolver::StrongRemove;
use p2panda_auth::group::{GroupAction, GroupMember};
use p2panda_auth::processor::AuthExtension;

use crate::error::{AddContactError, Error};
use crate::filesystem::Filesystem;
use chrono::{Duration, Utc};
use futures::Stream;
use named_id::Rename;
use named_id::*;
use p2panda_core::{Body, Hash, PublicKey};
use p2panda_spaces::ActorId;
use p2panda_store::{LogStore, SqliteStore};
use p2panda_stream::IngestExt;
use p2panda_stream::partial::operations::PartialOrder;
use tokio::sync::mpsc;

use mailbox_client::manager::{Mailboxes, MailboxesConfig};

use crate::chat::ChatMessageContent;
use crate::contact::{InboxTopic, QrCode, ShareIntent};
use crate::local_store::NodeData;
use crate::mailbox::MailboxOperation;
use crate::payload::{
    AnnouncementsPayload, ChatPayload, Extensions, InboxPayload, Payload, Profile,
};
use crate::stores::OpStore;
use crate::topic::{Topic, TopicId};
use crate::{
    AgentId, AsBody, ChatId, ChatReaction, DashAction, DeviceGroupId, DeviceGroupPayload, DeviceId,
    DirectChatId, Header, Operation,
};

pub use crate::local_store::LocalStore;
pub use stream_processing::Notification;

pub type NodeOpStore = OpStore<SqliteStore<TopicId, Extensions>>;
// pub type NodeOpStore = OpStore<p2panda_store::MemoryStore<TopicId, Extensions>>;

#[derive(Clone, Debug)]
pub struct NodeConfig {
    pub contact_code_expiry: Duration,
    pub mailboxes_config: MailboxesConfig,
}

impl NodeConfig {
    #[cfg(feature = "testing")]
    pub fn testing() -> Self {
        let mut mailboxes_config = MailboxesConfig::default();
        mailboxes_config.active_interval = std::time::Duration::from_millis(500);
        mailboxes_config.degraded_interval = std::time::Duration::from_millis(500);
        mailboxes_config.stopped_interval = std::time::Duration::from_millis(5000);
        mailboxes_config.between_polls_delay = std::time::Duration::from_millis(10);
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

pub type Orderer<S> =
    PartialOrder<TopicId, Extensions, S, p2panda_stream::partial::MemoryStore<p2panda_core::Hash>>;

#[derive(Clone)]
pub(crate) struct CancelAndWait<R> {
    handle: Arc<tokio::sync::Mutex<Option<tokio::task::JoinHandle<R>>>>,
    token: tokio_util::sync::CancellationToken,
}

impl<R> CancelAndWait<R> {
    pub fn new(
        handle: tokio::task::JoinHandle<R>,
        token: tokio_util::sync::CancellationToken,
    ) -> Self {
        Self {
            handle: Arc::new(tokio::sync::Mutex::new(Some(handle))),
            token,
        }
    }

    pub async fn cancel_and_wait(self) -> Option<Result<R, tokio::task::JoinError>> {
        self.token.cancel();
        Some(self.handle.lock().await.take()?.await)
    }
}

#[derive(Clone)]
pub struct Node {
    pub op_store: NodeOpStore,

    pub mailboxes: Mailboxes<MailboxOperation, NodeOpStore>,

    // groups: p2panda_auth::group::Groups,
    config: NodeConfig,
    notification_tx: Option<mpsc::Sender<Notification>>,

    /// Add new subscription streams
    stream_tx: mpsc::Sender<Pin<Box<dyn Stream<Item = Operation> + Send + 'static>>>,

    /// Abort handle for the stream processing background task
    stream_task: Option<CancelAndWait<()>>,

    pub(crate) local_store: LocalStore,
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
        let local_store = LocalStore::new(filesystem.local_store_path()).await?;
        let op_store = NodeOpStore::create(filesystem.op_store_path()).await?;
        Self::init(local_store, op_store, config, notification_tx).await
    }

    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all, fields(me = ?local_store.device_id().unwrap().renamed())))]
    pub(crate) async fn init(
        local_store: LocalStore,
        op_store: NodeOpStore,
        config: NodeConfig,
        notification_tx: Option<mpsc::Sender<Notification>>,
    ) -> Result<Self> {
        let node_data = local_store.node_data()?;

        let (stream_tx, stream_rx) = mpsc::channel(100);

        let mailboxes = Mailboxes::spawn(op_store.clone(), config.mailboxes_config.clone()).await?;

        let mut node = Self {
            op_store: op_store.clone(),
            mailboxes,
            config,
            local_store: local_store.clone(),
            node_data,
            notification_tx,
            stream_tx,
            stream_task: None,
        };

        node.stream_task = Some(node.spawn_stream_process_loop(stream_rx));

        node.initialize_device_group().await?;
        node.initialize_stored_topics().await?;

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
                tracing::warn!(
                    topic = ?topic.renamed(),
                    author = ?author.renamed(),
                    "No log found"
                );
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
    ) -> Result<QrCode, crate::Error> {
        let inbox_topic = if inbox {
            let inbox_topic = InboxTopic {
                topic: Topic::inbox(self.device_id()),
                expires_at: Utc::now() + self.config.contact_code_expiry,
            };
            self.initialize_topic(*inbox_topic.topic)
                .await
                .map_err(|err| crate::Error::InitializeTopic(format!("{err}")))?;
            self.local_store
                .add_active_inbox_topic(inbox_topic.clone())
                .map_err(|err| crate::Error::AddActiveInbox(format!("{err}")))?;
            Some(inbox_topic)
        } else {
            None
        };

        Ok(QrCode {
            device_pubkey: self.device_id(),
            inbox_topic,
            agent_id: self.node_data.agent_id,
            share_intent,
        })
    }

    pub fn agent_id(&self) -> AgentId {
        self.node_data.agent_id
    }

    pub fn device_id(&self) -> DeviceId {
        self.node_data.device_id()
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
        Topic::direct_chat([me, other])
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
        mut initial_agents: BTreeMap<AgentId, Access>,
    ) -> anyhow::Result<ChatId> {
        let chat_id = Topic::random();

        let agents_to_invite = initial_agents.keys().copied().collect::<Vec<_>>();

        // The creator must always have Manage access
        initial_agents.insert(self.agent_id(), Access::manage());

        let mut initial_members = initial_agents
            .iter()
            .map(|(m, a)| (m.to_group_member(), *a))
            .collect::<BTreeMap<_, _>>();

        #[cfg(feature = "auth-workaround")]
        {
            for (agent, access) in initial_agents
                .iter()
                .filter(|(_, access)| **access >= Access::manage())
            {
                let Some(device_id) = self.local_store.lookup_contact_device(*agent)? else {
                    continue;
                };
                // Add in all devices with manage access in the device group
                initial_members.insert(device_id.to_group_member(), access.clone());
            }
        }

        let initial_members = initial_members.into_iter().collect::<Vec<_>>();

        tracing::info!(members = ?initial_members.clone().renamed(), "new group created with members");

        self.author_operation(
            chat_id,
            DashAction::group_action(chat_id, GroupAction::Create { initial_members }),
            Some(&format!("create_group({})", chat_id.renamed())),
        )
        .await?;

        self.register_topic(chat_id).await?;

        for agent in agents_to_invite {
            self.invite_to_group(chat_id, agent).await?;
        }
        Ok(chat_id)
    }

    pub async fn add_group_member(
        &self,
        chat_id: ChatId,
        agent_id: AgentId,
        access: p2panda_auth::Access,
    ) -> anyhow::Result<()> {
        let mut members = vec![(agent_id.to_group_member(), access)];

        // HACK: remove this block once p2panda-auth supports Manage access for Groups
        #[cfg(feature = "auth-workaround")]
        {
            // Add in all devices with manage access in the device group
            if access >= Access::manage() {
                if let Some(device_id) = self.local_store.lookup_contact_device(agent_id)? {
                    members.push((device_id.to_group_member(), access));
                };
            }
        }

        tracing::info!(members = ?members.clone().renamed(), "members added to existing group");

        for (member, access) in members {
            self.author_operation(
                chat_id,
                DashAction::group_action(
                    chat_id,
                    GroupAction::Add {
                        member,
                        access: access.clone(),
                    },
                ),
                Some(&format!(
                    "add_group_member({}, {})",
                    chat_id.renamed(),
                    member.renamed()
                )),
            )
            .await?;
        }

        self.invite_to_group(chat_id, agent_id).await?;

        Ok(())
    }

    #[cfg(not(feature = "auth-workaround"))]
    pub async fn remove_group_member(
        &self,
        chat_id: ChatId,
        agent_id: AgentId,
    ) -> anyhow::Result<()> {
        unimplemented!(
            "Removing group members cannot be accomplished until p2panda-auth supports Manage access for Groups (when the `auth-workaround` feature is removed)"
        );
        self.author_operation(
            chat_id,
            DashAction::group_action(
                chat_id,
                GroupAction::Remove {
                    member: agent_id.to_group_member(),
                },
            ),
            Some(&format!(
                "remove_group_member({}, {})",
                chat_id.renamed(),
                agent_id.renamed()
            )),
        )
        .await?;
        Ok(())
    }

    pub async fn get_group_members(
        &self,
        chat_id: ChatId,
    ) -> anyhow::Result<BTreeSet<(PublicKey, Access)>> {
        let members = self.local_store.chat_group_members(chat_id).await?;
        Ok(members)
    }

    /// "Joining" a chat means subscribing to messages for that chat.
    /// This needs to be accompanied by being added as a member of the chat Space by an existing member
    /// -- you're not fully a member until someone adds you.
    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all, parent = None, fields(me = ?self.device_id().renamed())))]
    pub async fn join_group(&self, chat_id: ChatId) -> anyhow::Result<()> {
        tracing::debug!(?chat_id, "joined group");
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

        // TODO: need to filter messages by actual DeviceIds in the Agent's DeviceGroup
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
    pub async fn shutdown(mut self) {
        if let Some(cancel_and_wait) = self.stream_task.take() {
            cancel_and_wait.cancel_and_wait().await;
        }
    }

    /// Store someone as a contact, and:
    /// - register their spaces keybundle so we can add them to spaces
    /// - subscribe to their inbox
    /// - store them in the contacts map
    /// - send an invitation to them to do the same
    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all, fields(me = ?self.device_id().renamed())))]
    pub async fn add_contact(&self, contact: QrCode) -> Result<AgentId, AddContactError> {
        tracing::debug!("adding contact: {:?}", contact);

        #[cfg(feature = "auth-workaround")]
        self.local_store
            .save_contact(contact.clone())
            .map_err(|e| AddContactError::StoreContact(e.to_string()))?;

        // TODO: SPACES: Register the member in the spaces manager

        self.register_topic(Topic::announcements(contact.agent_id))
            .await
            .map_err(|e| Error::InitializeTopic(e.to_string()))?;

        // Must subscribe to the new member's device group in order to receive their
        // group control messages.

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

        let agent = contact.agent_id;
        let direct_topic = self.direct_chat_topic(agent);
        self.register_topic(direct_topic)
            .await
            .map_err(|e| Error::InitializeTopic(e.to_string()))?;

        self.author_operation(
            self.device_group_topic(),
            Payload::DeviceGroup(DeviceGroupPayload::AddContact(contact.clone())),
            Some(&format!("add_contact/invitation({})", agent.renamed())),
        )
        .await
        .map_err(|e| Error::AuthorOperation(e.to_string()))?;

        // This is only run by the person who scanned the QR code.
        if let Some(inbox_topic) = contact.inbox_topic.clone() {
            // Note, the contact won't send anything back,
            // but we need to subscribe so we can sync with the contact
            self.initialize_topic(*inbox_topic.topic)
                .await
                .map_err(|e| Error::InitializeTopic(e.to_string()))?;

            // Create the code to send back (with my info)
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

            // Author it so the QR code creator will get it on the inbox topic
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

    async fn initialize_stored_topics(&self) -> anyhow::Result<()> {
        self.initialize_topic(*Topic::announcements(self.agent_id()))
            .await?;

        for topic in self.local_store.get_active_inbox_topics()?.iter() {
            self.initialize_topic(*topic.topic.clone()).await?;
        }

        for topic in self.local_store.subscribed_topics()?.iter() {
            self.initialize_topic(*topic).await?;
        }

        Ok(())
    }

    async fn initialize_device_group(&self) -> anyhow::Result<bool> {
        let topic = Topic::announcements(self.agent_id());
        let log = self.get_log(topic.into(), self.device_id()).await?;

        let initialized = log.iter().any(|(header, _)| {
            let Some(auth) = header.extension::<AuthExtension>() else {
                return false;
            };
            auth.group_id == self.agent_id().to_group_member().id()
                && matches!(auth.action, GroupAction::Create { .. })
        });

        if initialized {
            return Ok(false);
        }

        self.author_operation(
            topic,
            DashAction::GroupControl(AuthExtension {
                group_id: self.agent_id().to_group_member().id(),
                action: GroupAction::Create {
                    initial_members: vec![(self.device_id().to_group_member(), Access::manage())],
                },
            }),
            Some("initialize_device_group"),
        )
        .await?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use crate::testing::TestNode;

    use super::*;

    #[tokio::test]
    async fn test_initialize_device_group() {
        let node = TestNode::new(NodeConfig::default(), "test_node").await;
        let did_initialize = node.initialize_device_group().await.unwrap();

        // The device group should already be initialized and should not happen twice.
        assert!(!did_initialize);
    }
}
