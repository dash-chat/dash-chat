pub(crate) mod actor;
mod app_processing;
pub(crate) mod publish;

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, LazyLock};

use crate::compat::Capabilities;
use crate::error::{AddContactError, Error, ShutdownError};
use crate::filesystem::Filesystem;
use crate::node::actor::{Actor, Command};
use aliased::Aliasing;
use anyhow::Result;
use chrono::{Duration, Utc};
use dashchat_compat::VersionConvert;
use p2panda::network::MdnsDiscoveryMode;
use p2panda::operation::{Header, Operation};
use p2panda::{Hash, NetworkId, Node as P2PandaNode, NodeId, RelayUrl, VerifyingKey};
use p2panda_auth::Access;
use p2panda_auth::group::resolver::StrongRemove;
use p2panda_auth::group::{GroupAction, GroupMember};
use p2panda_spaces::ActorId;
use tokio::sync::{Mutex, mpsc, oneshot};

use mailbox_client::manager::{Mailboxes, MailboxesConfig};
use tokio::task::JoinHandle;

use crate::chat::ChatMessageContent;
use crate::contact::{InboxTopic, QrCode, ShareIntent};
use crate::mailbox::MailboxOperation;
use crate::payload::{AnnouncementsPayload, ChatPayload, InboxPayload, Payload, Profile};
use crate::stores::{GroupStore, LocalStore, NodeKeys, OpStore};
use crate::topic::{Topic, TopicId};
use crate::{
    AgentId, ChatId, ChatReaction, DeviceGroupId, DeviceGroupPayload, DeviceId, DirectChatId,
};

pub use app_processing::Notification;

const NETWORK_ID: &'static str = "dash-chat";

pub static RELAY_URL: LazyLock<RelayUrl> = LazyLock::new(|| {
    "https://euc1-1.relay.n0.iroh-canary.iroh.link"
        .parse()
        .expect("valid relay URL")
});

#[derive(Clone, Debug)]
pub struct NodeConfig {
    pub contact_code_expiry: Duration,
    pub mailboxes_config: MailboxesConfig,
    pub capabilities: Capabilities,
    pub network_id: NetworkId,
    pub mdns_mode: MdnsDiscoveryMode,
    pub relay_url: Option<RelayUrl>,
}

impl NodeConfig {
    #[cfg(feature = "testing")]
    pub fn testing() -> Self {
        use crate::compat::Capabilities;

        let mut mailboxes_config = MailboxesConfig::default();
        mailboxes_config.active_interval = std::time::Duration::from_millis(1000);
        mailboxes_config.degraded_interval = std::time::Duration::from_millis(2000);
        mailboxes_config.stopped_interval = std::time::Duration::from_millis(5000);
        mailboxes_config.between_polls_delay = std::time::Duration::from_millis(100);
        Self {
            contact_code_expiry: Duration::days(7),
            mailboxes_config,
            capabilities: Capabilities::current(),
            network_id: Hash::digest(NETWORK_ID.as_bytes()).into(),
            // In testing we disable mDNS discovery and do not provide a relay address so as not
            // to effect expected behavior of existing tests.
            mdns_mode: MdnsDiscoveryMode::Disabled,
            relay_url: None,
        }
    }
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            contact_code_expiry: Duration::days(7),
            mailboxes_config: MailboxesConfig::default(),
            capabilities: Capabilities::current(),
            network_id: Hash::digest(NETWORK_ID.as_bytes()).into(),
            mdns_mode: MdnsDiscoveryMode::Active,
            relay_url: Some(RELAY_URL.clone()),
        }
    }
}

pub type DashResolver = StrongRemove<VerifyingKey, Hash, Operation, ()>;

#[derive(Clone)]
pub struct Node {
    pub op_store: OpStore,

    pub mailboxes: Mailboxes<MailboxOperation, OpStore>,

    config: NodeConfig,

    notification_tx: Option<mpsc::Sender<Notification>>,
    topic_subscribed_tx: Option<mpsc::Sender<TopicId>>,

    actor_tx: mpsc::Sender<Command>,
    processor_cancel_tx: mpsc::Sender<()>,
    processor_handle: Arc<Mutex<Option<JoinHandle<()>>>>,

    /// All bootstrap nodes we have registered on our node.
    ///
    /// These are kept to avoid redundant duplicate calls to node.insert_bootstrap.
    registered_bootstraps: Arc<Mutex<HashSet<(NodeId, RelayUrl)>>>,

    pub local_store: LocalStore,
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

    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all, fields(me = ?node_keys.device_id().aliased())))]
    pub async fn init(
        filesystem: Filesystem,
        local_store: LocalStore,
        node_keys: NodeKeys,
        config: NodeConfig,
        notification_tx: Option<mpsc::Sender<Notification>>,
        topic_subscribed_tx: Option<mpsc::Sender<TopicId>>,
    ) -> Result<Self> {
        // === p2panda node === //

        let url = format!("sqlite://{}", filesystem.op_store_path().to_string_lossy());
        let mut builder = P2PandaNode::builder()
            .network_id(config.network_id)
            .signing_key(node_keys.private_key.clone())
            .database_url(&url)
            .mdns_mode(config.mdns_mode.clone());

        if let Some(relay_url) = &config.relay_url {
            builder = builder.relay_url(relay_url.clone());
        }

        let p2panda_node = builder.spawn().await?;
        // @TODO: the store() method is behind the "test_utils" feature flag, if we actually do
        // need access to the store then we should make this method public.
        let store = p2panda_node.store();

        // Spawn node actor.
        let (node_actor, events_rx) = Actor::new(p2panda_node);
        let actor_tx = node_actor.spawn().await?;

        // === stores === //

        // @TODO: I didn't look too closely into if these stores are needed yet. It's possible we
        // can reduce the need for these wrappers by either moving generally useful methods into
        // p2panda or finding alternative routes to achieve the same queries.
        let group_store = GroupStore::new(store.clone());
        let op_store = OpStore::from_sqlite(store.clone());

        // === mailboxes === //

        let sync_tracker = std::sync::Arc::new(
            mailbox_client::sync_tracker::MailboxSyncTracker::open(
                filesystem.mailbox_sync_tracker_path(),
            )
            .await?,
        );

        let mailboxes = Mailboxes::spawn(
            op_store.clone(),
            sync_tracker,
            config.mailboxes_config.clone(),
        )
        .await?;

        // === node === //

        let (processor_cancel_tx, processor_cancel_rx) = mpsc::channel(1);
        let node = Self {
            op_store,
            mailboxes,
            config,
            filesystem,
            local_store: local_store.clone(),
            group_store,
            node_keys,
            notification_tx,
            topic_subscribed_tx,
            actor_tx,
            processor_cancel_tx,
            processor_handle: Default::default(),
            registered_bootstraps: Default::default(),
        };

        // === application processor task === //

        let processor_handle =
            node.spawn_application_processor_task(events_rx, processor_cancel_rx);
        node.processor_handle.lock().await.replace(processor_handle);

        // === topics === //

        node.initialize_stored_topics().await?;

        // === announce === //

        node.announce_device_capabilities(node.config.capabilities)
            .await?;

        Ok(node)
    }

    pub fn data_path(&self) -> &PathBuf {
        self.filesystem.data_path()
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
                topic: Topic::inbox()
                    .alias_named(&format!("inbox({:?})", self.device_id().aliased())),
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
            topic.alias_named(&format!("direct({:?},{:?})", other.aliased(), me.aliased()))
        } else {
            topic.alias_named(&format!("direct({:?},{:?})", me.aliased(), other.aliased()))
        }
    }

    /// Create a new direct chat Space.
    /// Note that only one node should create the space!
    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all, fields(me = ?self.device_id().aliased())))]
    pub async fn create_direct_chat_space(&self, other: AgentId) -> anyhow::Result<()> {
        let topic = self.direct_chat_topic(other);

        let my_actor = self.agent_id();
        self.register_topic(topic).await?;

        let other_device_id = self
            .local_store
            .lookup_contact_by_agent_id(other)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Contact not found in lookup table"))?;

        // TODO: this should use a transaction, but the race is not a big deal here
        let deps = self.group_store.heads(*topic).await?;
        let initial_members = vec![
            (GroupMember::Individual(*self.device_id()), Access::write()),
            (GroupMember::Individual(*other_device_id), Access::write()),
        ];
        self.publish(
            topic,
            Payload::group_control(topic, GroupAction::Create { initial_members }, deps)?,
            Some(&format!("create_direct_chat_space({:?})", topic.aliased())),
        )
        .await?;

        tracing::info!(
            my_actor = ?my_actor.aliased(),
            other = ?other.aliased(),
            topic = ?topic.aliased(),
            "creating direct chat space"
        );

        tracing::info!(?topic, ?topic, "created direct chat space");

        Ok(())
    }

    pub async fn create_group(
        &self,
        mut initial_members: BTreeMap<VerifyingKey, p2panda_auth::Access>,
    ) -> anyhow::Result<ChatId> {
        let chat_id = Topic::random();
        tracing::info!(
            me = ?self.device_id().aliased(),
            chat_id = ?chat_id.aliased(),
            member_count = initial_members.len(),
            "creating group"
        );

        let device_ids: Vec<DeviceId> = initial_members
            .keys()
            .map(|verifying_key| DeviceId::from(*verifying_key))
            .collect();

        let contacts = self.local_store.lookup_contacts(&device_ids).await?;
        let device_to_agent: BTreeMap<DeviceId, AgentId> = device_ids
            .iter()
            .filter_map(|did| match contacts.get(did) {
                Some(agent) => Some((*did, *agent)),
                None => {
                    tracing::warn!(
                        "Contact not found (when creating group): {:?}",
                        did.aliased()
                    );
                    None
                }
            })
            .collect();
        let agents: Vec<AgentId> = device_to_agent.values().copied().collect();

        // The creator must always have Manage access
        initial_members.insert(*self.device_id(), p2panda_auth::Access::manage());

        let initial_members: Vec<_> = initial_members
            .into_iter()
            .map(|(verifying_key, access)| (GroupMember::Individual(verifying_key), access))
            .collect();

        self.register_topic(chat_id).await?;

        // TODO: this should use a transaction, but the race is not a big deal here
        let deps = self.group_store.heads(*chat_id).await?;
        self.publish(
            chat_id,
            Payload::group_control(chat_id, GroupAction::Create { initial_members }, deps)?,
            Some(&format!("create_group({:?})", chat_id.aliased())),
        )
        .await?;

        // Ensure that future non-contact members can see every initial member's
        // profile, including the creator's. The creator is the only node that
        // knows all of these agent_ids (initial members are by definition the
        // creator's contacts), so we publish them here.
        let mut introduced_agents = device_to_agent.clone();
        introduced_agents.insert(self.device_id(), self.agent_id());
        self.introduce_agents_to_group(chat_id, introduced_agents)
            .await?;

        self.local_store.save_group_chat_subscribed(chat_id).await?;
        self.initialize_topic(*chat_id).await?;

        for agent in agents {
            self.invite_to_group(chat_id, agent).await?;
        }
        Ok(chat_id)
    }

    /// This is a temporary hack until we have device groups, see docs for [`ChatPayload::IntroduceAgents`].
    async fn introduce_agents_to_group(
        &self,
        chat_id: ChatId,
        agents: BTreeMap<DeviceId, AgentId>,
    ) -> anyhow::Result<()> {
        self.publish(
            chat_id,
            Payload::Chat(ChatPayload::IntroduceAgents { agents }),
            Some(&format!("introduce_to_group({:?})", chat_id.aliased())),
        )
        .await?;
        Ok(())
    }

    async fn invite_to_group(&self, chat_id: ChatId, person: AgentId) -> anyhow::Result<()> {
        let payload = Payload::Chat(ChatPayload::JoinGroup { chat_id });
        tracing::info!(
            "{:?} is inviting {:?} to group {:?}",
            self.device_id().aliased(),
            person.aliased(),
            chat_id.aliased(),
        );
        self.publish(
            self.direct_chat_topic(person),
            payload,
            Some(&format!(
                "invite_to_group({:?}, {:?})",
                chat_id.aliased(),
                person.aliased()
            )),
        )
        .await?;
        Ok(())
    }

    pub async fn add_group_member(
        &self,
        chat_id: ChatId,
        member: VerifyingKey,
        access: p2panda_auth::Access,
    ) -> anyhow::Result<()> {
        // TODO: this should use a transaction, but the race is not a big deal here
        let deps = self.group_store.heads(*chat_id).await?;

        if deps.is_empty() {
            return Err(anyhow::anyhow!(
                "group must be known locally before adding member: {chat_id:?}"
            ));
        }

        self.publish(
            chat_id,
            Payload::group_control(
                chat_id,
                GroupAction::Add {
                    member: GroupMember::Individual(member),
                    access,
                },
                deps,
            )?,
            Some(&format!("add_group_member({:?})", chat_id.aliased())),
        )
        .await?;

        let agent_id = self
            .local_store
            .lookup_contact_by_device_id(DeviceId::from(member))
            .await?;
        if let Some(agent_id) = agent_id {
            // Tell existing group members about the new member's agent_id so
            // they can subscribe to its announcements topic and see its profile,
            // even before the new member has come online to announce itself.
            self.introduce_agents_to_group(
                chat_id,
                BTreeMap::from([(DeviceId::from(member), agent_id)]),
            )
            .await?;
            self.invite_to_group(chat_id, agent_id).await?;
        } else {
            tracing::warn!(
                "Contact not found (when adding group member): {:?}",
                DeviceId::from(member).aliased()
            );
        }

        Ok(())
    }

    pub async fn remove_group_member(
        &self,
        chat_id: ChatId,
        member: VerifyingKey,
    ) -> anyhow::Result<()> {
        // TODO: this should use a transaction, but the race is not a big deal here
        let deps = self.group_store.heads(*chat_id).await?;
        self.publish(
            chat_id,
            Payload::group_control(
                chat_id,
                GroupAction::Remove {
                    member: GroupMember::Individual(member),
                },
                deps,
            )?,
            Some(&format!("remove_group_member({:?})", chat_id.aliased())),
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
    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all, parent = None, fields(me = ?self.device_id().aliased())))]
    pub async fn join_group(&self, chat_id: ChatId) -> anyhow::Result<()> {
        tracing::info!(?chat_id, "joined group");
        self.register_topic(chat_id).await
    }

    pub async fn get_groups(&self) -> anyhow::Result<Vec<ChatId>> {
        self.local_store.get_group_chat_ids().await
    }

    pub async fn set_profile(&self, profile: Profile) -> Result<Header, crate::Error> {
        let header = self
            .publish(
                Topic::announcements(self.agent_id()),
                Payload::Announcements(AnnouncementsPayload::SetProfile(profile)),
                Some(&format!("set_profile({:?})", self.device_id().aliased())),
            )
            .await
            .map_err(|e| Error::AuthorOperation(e.to_string()))?;

        Ok(header)
    }

    pub async fn my_profile(&self) -> anyhow::Result<Option<Profile>> {
        self.local_store.get_profile(self.agent_id()).await
    }

    pub async fn lookup_contact(&self, device_id: DeviceId) -> anyhow::Result<Option<AgentId>> {
        self.local_store
            .lookup_contact_by_device_id(device_id)
            .await
    }

    pub async fn all_contact_agent_ids(&self) -> anyhow::Result<Vec<AgentId>> {
        self.local_store.all_contact_agent_ids().await
    }

    pub async fn subscribed_topics(&self) -> anyhow::Result<std::collections::BTreeSet<TopicId>> {
        self.local_store.subscribed_topics().await
    }

    /// Get all messages for a chat from the logs.
    ///
    /// In the real app, the interleaving of logs happens on the front end.
    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all, fields(me = ?self.device_id().aliased())))]
    #[cfg(feature = "testing")]
    pub async fn get_messages(
        &self,
        topic: impl Into<ChatId>,
    ) -> anyhow::Result<Vec<crate::chat::testing::ChatMessage>> {
        let chat_id = topic.into();
        let mut messages = vec![];

        let authors = self.op_store.get_authors(chat_id.into()).await?;

        for (header, payload) in self
            .op_store
            .get_interleaved_logs(chat_id.into(), authors.into_iter().collect())
            .await?
        {
            if let Some(Payload::Chat(ChatPayload::Message(message))) = payload {
                messages.push(crate::chat::testing::ChatMessage::new(message, &header));
            }
        }

        Ok(messages)
    }

    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all, fields(me = ?self.device_id().aliased())))]
    pub async fn send_message(
        &self,
        topic: impl Into<ChatId>,
        message: ChatMessageContent,
    ) -> anyhow::Result<Header> {
        let topic = topic.into();

        let (capabilities, num_agents) = self.get_group_capabilities(topic).await?;
        let capabilities = capabilities.ok_or(anyhow::anyhow!(
            "no capabilities found for chat: {:?}",
            topic.aliased()
        ))?;

        // NOTE: we may need logic for an agent to re-send a downgraded message if they later discover
        //       intended recipients who don't have the proper capabilities for receiving this message
        if num_agents == 1 {
            tracing::warn!(
                "sending message to group without knowing any other members' capabilities",
            );
        }
        let message = message.to_version(&capabilities)?;

        let header = self
            .publish(
                topic,
                Payload::Chat(ChatPayload::Message(message.clone())),
                None,
            )
            .await?;

        Ok(header)
    }

    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all, fields(me = ?self.device_id().aliased())))]
    pub async fn add_reaction(
        &self,
        topic: impl Into<ChatId>,
        reaction: ChatReaction,
    ) -> anyhow::Result<Header> {
        let topic = topic.into();
        let header = self
            .publish(topic, Payload::Chat(ChatPayload::Reaction(reaction)), None)
            .await?;

        Ok(header)
    }

    /// Abort the stream processing background task, allowing database handles to be released.
    pub async fn shutdown(&self) -> Result<(), ShutdownError> {
        // Stop polling mailboxes so the manager loop stops issuing OpStore queries.
        self.mailboxes.clear().await;

        let (reply_tx, reply_rx) = oneshot::channel();
        if let Err(err) = self.actor_tx.send(Command::Shutdown { reply_tx }).await {
            tracing::warn!("failed to send shutdown command to node actor: {}", err);
            return Err(ShutdownError::ActorShutdown(Box::new(err)));
        }

        reply_rx.await?;

        if let Err(err) = self.processor_cancel_tx.send(()).await {
            tracing::warn!(
                "failed to send cancel signal to application processor: {}",
                err
            );
            return Err(ShutdownError::ActorShutdown(Box::new(err)));
        }

        if let Some(handle) = self.processor_handle.lock().await.take() {
            let _ = handle.await;
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
    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all, fields(me = ?self.device_id().aliased())))]
    pub async fn add_contact(&self, contact: QrCode) -> Result<AgentId, AddContactError> {
        tracing::debug!(
            device_pub_key = ?contact.device_pubkey.aliased(),
            agent_id = ?contact.agent_id.aliased(),
            inbox_topic = ?contact.inbox_topic,
            "adding contact",
        );

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

        let agent = contact.agent_id;
        let direct_topic = self.direct_chat_topic(agent);
        self.register_topic(direct_topic)
            .await
            .map_err(|e| Error::InitializeTopic(e.to_string()))?;

        self.publish(
            self.device_group_topic(),
            Payload::DeviceGroup(DeviceGroupPayload::AddContact(contact.clone())),
            Some(&format!("add_contact/add_contact({:?})", agent.aliased())),
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
            self.publish(
                inbox_topic.topic,
                Payload::Inbox(InboxPayload::ContactRequest { code, profile }),
                Some(&format!(
                    "add_contact/contact_request({:?})",
                    agent.aliased()
                )),
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
    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all, fields(me = ?self.device_id().aliased())))]
    pub async fn reject_contact_request(&self, agent_id: AgentId) -> Result<(), Error> {
        tracing::debug!("rejecting contact request from: {:?}", agent_id);

        self.publish(
            self.device_group_topic(),
            Payload::DeviceGroup(DeviceGroupPayload::RejectContactRequest(agent_id)),
            Some(&format!("reject_contact_request({:?})", agent_id.aliased())),
        )
        .await
        .map_err(|e| Error::AuthorOperation(e.to_string()))?;

        Ok(())
    }

    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all, fields(me = ?self.device_id().aliased())))]
    pub async fn remove_contact(&self, _chat_actor_id: ActorId) -> anyhow::Result<()> {
        // TODO: shutdown inbox task, etc.
        todo!("add tombstone to contacts list");
    }

    /// Mark messages as read by storing a ReadMessages operation in the device group topic.
    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all, fields(me = ?self.device_id().aliased())))]
    pub async fn mark_messages_read(
        &self,
        chat_id: ChatId,
        message_hashes: Vec<Hash>,
    ) -> Result<(), Error> {
        use crate::payload::ReadMessagesPayload;

        self.publish(
            self.device_group_topic(),
            Payload::DeviceGroup(DeviceGroupPayload::ReadMessages(ReadMessagesPayload {
                chat_id,
                message_hashes,
            })),
            Some(&format!("mark_messages_read({:?})", chat_id.aliased())),
        )
        .await
        .map_err(|e| Error::AuthorOperation(e.to_string()))?;

        Ok(())
    }

    async fn initialize_stored_topics(&self) -> anyhow::Result<()> {
        self.initialize_topic(
            *Topic::announcements(self.agent_id())
                .alias_named(&format!("announce({:?})", self.agent_id().aliased())),
        )
        .await?;

        for topic in self.local_store.get_active_inbox_topics().await?.iter() {
            self.initialize_topic(
                *topic
                    .topic
                    .clone()
                    .alias_named(&format!("inbox({:?})", self.device_id().aliased())),
            )
            .await?;
        }

        for topic in self.local_store.subscribed_topics().await?.iter() {
            self.initialize_topic(*topic).await?;
        }

        // @TODO: I had to add this so that the device group topic is subscribed to when we later
        // attempt to publish operations to it.
        self.initialize_topic(self.device_group_topic().into())
            .await?;

        Ok(())
    }

    pub async fn announce_device_capabilities(
        &self,
        capabilities: Capabilities,
    ) -> anyhow::Result<()> {
        let announcements = Topic::announcements(self.agent_id());
        let latest_capability = self.local_store.get_capabilities(self.device_id()).await?;

        // If the capability is unset or different from the current one, set it now.
        if latest_capability != Some(capabilities) {
            self.publish(
                announcements,
                Payload::Announcements(AnnouncementsPayload::SetCapabilities { capabilities }),
                Some(&format!(
                    "set_device_capabilities({:?})",
                    self.device_id().aliased()
                )),
            )
            .await?;
        }

        Ok(())
    }

    /// Find the infimum of the capabilities of all other members of the group with read access or above.
    ///
    /// This is dependent on eventual consistency, and as other members join, the capabilities may change.
    pub async fn get_group_capabilities(
        &self,
        topic: ChatId,
    ) -> anyhow::Result<(Option<Capabilities>, usize)> {
        let members = self.group_store.members(topic).await?;
        let mut devices = members
            .iter()
            .filter_map(|(member, access)| {
                // Only include members with read access or above
                // TODO: make sure this pubkey corresponds to a DeviceId and not an AgentId,
                //       once device groups are implemented
                (*access >= Access::read()).then_some(*member)
            })
            .collect::<BTreeSet<_>>();
        devices.insert(self.device_id());

        // Collect capabilities for all agents
        let caps = futures::future::join_all(
            devices
                .into_iter()
                .map(|device| self.local_store.get_capabilities(device)),
        )
        .await
        .into_iter()
        .collect::<Result<Vec<Option<Capabilities>>>>()?;

        let caps = caps.into_iter().flatten().collect::<Vec<_>>();
        let num = caps.len();

        Ok((caps.into_iter().reduce(|a, b| a.infimum(&b)), num))
    }
}
