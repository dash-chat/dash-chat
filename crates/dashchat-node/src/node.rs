pub(crate) mod actor;
mod app_processing;
pub(crate) mod publish;

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, LazyLock};

use crate::blob_sync::{BlobFetchConfig, BlobFetchPool, BlobSync};
use crate::compat::Capabilities;
use crate::error::{AddContactError, Error, RemoveGroupMemberError, ShutdownError};
use crate::filesystem::Filesystem;
use crate::node::actor::{Actor, Command};
use aliased::Aliasing;
use anyhow::{Context, Result};
use chrono::{Duration, Utc};
use dashchat_compat::VersionConvert;
use dashchat_utils::blob_sync::MAX_BLOB_BYTES;
use p2panda::network::MdnsDiscoveryMode;
use p2panda::operation::{Header, LogId, Operation};
use p2panda::{Hash, NetworkId, Node as P2PandaNode, NodeId, RelayUrl, VerifyingKey};
use p2panda_auth::Access;
use p2panda_auth::group::resolver::StrongRemove;
use p2panda_auth::group::{GroupAction, GroupMember};
use p2panda_spaces::ActorId;
use tokio::sync::{Mutex, mpsc, oneshot};

use mailbox_client::manager::{Mailboxes, MailboxesConfig};
use tokio::task::JoinHandle;

use crate::chat::{ChatMessageContent, ChatOp, ChatOpKind, validate_edit};
use crate::contact::{InboxTopic, QrCode, ShareIntent};
use crate::mailbox::MailboxOperation;
use crate::payload::{AnnouncementsPayload, ChatPayload, InboxPayload, Payload, Profile};
use crate::stores::{GroupStore, LocalStore, NodeKeys, OpStore};
use crate::topic::{Topic, TopicId};
use crate::{
    AgentId, AsBody, ChatId, ChatReaction, DeviceGroupId, DeviceGroupPayload, DeviceId,
    DirectChatId, EditMessageError, MediaBundle, MediaMetaKind, MediaMetadata, OutgoingFile,
    OutgoingMedia,
};
use dashchat_utils::NETWORK_ID;

pub use app_processing::Notification;

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
    pub blob_fetch: BlobFetchConfig,
}

impl NodeConfig {
    /// Disable p2p features and only use mailbox-based communication.
    pub fn no_p2p(mut self) -> Self {
        self.mdns_mode = MdnsDiscoveryMode::Disabled;
        self.relay_url = None;
        self
    }

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
            network_id: *NETWORK_ID,
            // In testing we disable mDNS discovery and do not provide a relay address so as not
            // to effect expected behavior of existing tests.
            mdns_mode: MdnsDiscoveryMode::Disabled,
            relay_url: None,
            // Retry blob downloads quickly so tests don't wait on the
            // production-scale pass interval.
            blob_fetch: BlobFetchConfig {
                pass_interval: std::time::Duration::from_secs(2),
                attempt_timeout: std::time::Duration::from_secs(10),
                ..BlobFetchConfig::default()
            },
        }
    }
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            contact_code_expiry: Duration::days(7),
            mailboxes_config: MailboxesConfig::default(),
            capabilities: Capabilities::current(),
            network_id: *NETWORK_ID,
            mdns_mode: MdnsDiscoveryMode::Active,
            relay_url: Some(RELAY_URL.clone()),
            blob_fetch: BlobFetchConfig::default(),
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
    blob_sync: BlobSync,
    blob_fetch_handle: Arc<Mutex<Option<JoinHandle<()>>>>,
}

/// Refuse to publish a media item larger than [`MAX_BLOB_BYTES`] so an honest
/// node never references a blob that the fetcher's own cap would reject.
fn ensure_blob_size(size: u64, _name: &str) -> anyhow::Result<()> {
    if size as u64 > MAX_BLOB_BYTES {
        anyhow::bail!("a media item is {size} bytes, exceeds {MAX_BLOB_BYTES} byte limit");
    }
    Ok(())
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
        let endpoint = p2panda_node.endpoint();

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

        // === blob sync === //

        let self_endpoint = iroh::EndpointId::from_bytes(node_keys.device_id().as_bytes())?;
        let source_lookup = crate::blob_sync::MixedSourceLookup::new(
            op_store.clone(),
            mailboxes.clone(),
            self_endpoint,
        );

        // LogId = blake3(topic.as_bytes()) is one-way and the op-store does not
        // persist TopicId alongside each operation, so we invert it by hashing
        // the topics we subscribe to (chat media lives in subscribed chat
        // topics). Without this the pool starts empty and a blob left
        // undownloaded at shutdown is never re-queued — only the live path adds
        // it — so it can never load again after a restart.
        let blob_fetch = BlobFetchPool::from_ops(
            op_store.get_all_operations_not_fully_sorted(),
            op_store.store.clone(),
        )
        .await?;
        let blob_sync = BlobSync::new(
            endpoint,
            filesystem.blobs_store_path(),
            blob_fetch,
            source_lookup,
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
            blob_sync,
            blob_fetch_handle: Default::default(),
        };

        // === application processor task === //

        let processor_handle =
            node.spawn_application_processor_task(events_rx, processor_cancel_rx);
        node.processor_handle.lock().await.replace(processor_handle);

        // === blob fetch loop === //

        let blob_fetch_handle = node
            .blob_sync
            .spawn_fetch_loop(node.config.blob_fetch.clone());
        node.blob_fetch_handle
            .lock()
            .await
            .replace(blob_fetch_handle);

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

    #[cfg(feature = "testing")]
    pub fn endpoint_id(&self) -> iroh::EndpointId {
        iroh::EndpointId::from_bytes(self.device_id().as_bytes())
            .expect("device id is a valid endpoint id")
    }

    #[cfg(feature = "testing")]
    /// The node's iroh-blobs protocol handle, sharing its blob store. An
    /// in-process mailbox uses this so relayed blobs land in—and are served
    /// from—the same store on the same endpoint as the node.
    pub fn blobs(&self) -> iroh_blobs::BlobsProtocol {
        self.blob_sync.blobs.clone()
    }

    #[cfg(feature = "testing")]
    /// The node's blob downloader, for an in-process mailbox to fetch blobs
    /// into the shared store over the node's endpoint.
    pub fn blob_downloader(&self) -> iroh_blobs::api::downloader::Downloader {
        self.blob_sync.downloader()
    }

    #[cfg(feature = "testing")]
    /// Topics the blob fetch pool currently associates with `hash`. Lets a test
    /// assert that startup hydration re-queued a stored op's blob.
    pub async fn blob_fetch_pool_topics_for(&self, hash: iroh_blobs::Hash) -> Vec<TopicId> {
        self.blob_sync.fetch_pool.topics_for(hash).await
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
    ) -> Result<(), RemoveGroupMemberError> {
        // TODO: this should use a transaction, but the race is not a big deal here
        let member_id = DeviceId::from(member);
        if !self.has_other_admins(chat_id, member_id).await?
            && !self.is_only_member(chat_id, member_id).await?
        {
            return Err(RemoveGroupMemberError::LastAdmin);
        }

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

    async fn is_only_member(&self, chat_id: ChatId, member: DeviceId) -> anyhow::Result<bool> {
        let found_other_member = self
            .get_group_members(chat_id)
            .await?
            .iter()
            .any(|(m, _)| *m != member);

        Ok(!found_other_member)
    }

    async fn has_other_admins(&self, chat_id: ChatId, exclude: DeviceId) -> anyhow::Result<bool> {
        let result = self
            .get_group_members(chat_id)
            .await?
            .iter()
            .any(|(member, access)| {
                *access == p2panda_auth::Access::manage() && *member != exclude
            });

        Ok(result)
    }

    /// "Joining" a chat means subscribing to messages for that chat.
    /// This needs to be accompanied by being added as a member of the chat Space by an existing member
    /// -- you're not fully a member until someone adds you.
    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all, parent = None, fields(me = ?self.device_id().aliased())))]
    pub async fn join_group(&self, chat_id: ChatId) -> anyhow::Result<()> {
        tracing::info!(?chat_id, "joined group");
        self.register_topic(chat_id).await?;
        self.local_store.save_group_chat_subscribed(chat_id).await
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
        message: impl Into<String>,
        media: Option<OutgoingMedia>,
    ) -> anyhow::Result<Header> {
        let chat_id: ChatId = topic.into();
        let meta = if let Some(media) = media {
            Some(self.store_media(chat_id.into(), media).await?)
        } else {
            None
        };
        let message = ChatMessageContent::new(message, meta);
        self.send_message_raw(chat_id, message).await
    }

    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all, fields(me = ?self.device_id().aliased())))]
    pub async fn send_message_raw(
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

    /// Edit the text content of a previously-sent message.
    ///
    /// `edit_hash` must refer to a `Message` or `EditMessage` operation in this
    /// chat authored by us, within the edit window, and not already edited. The
    /// edit is validated before publishing; see [`EditError`](crate::EditError).
    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all, fields(me = ?self.device_id().aliased())))]
    pub async fn edit_message(
        &self,
        topic: impl Into<ChatId>,
        edit_hash: Hash,
        message: impl Into<String>,
    ) -> Result<Header, EditMessageError> {
        let topic = topic.into();
        let ops = self.chat_ops(topic).await?;
        let now = u64::from(p2panda_core::Timestamp::now());
        validate_edit(&ops, &edit_hash, self.device_id(), now, None)?;

        let header = self
            .publish(
                topic,
                Payload::Chat(ChatPayload::EditMessage {
                    message: message.into(),
                    edit_hash,
                }),
                None,
            )
            .await?;

        Ok(header)
    }

    /// Publish an edit without validating it. For testing the receiving-side
    /// handling of invalid edits, which the author-side validation would
    /// otherwise prevent from ever being created.
    #[cfg(any(test, feature = "testing"))]
    pub async fn edit_message_raw(
        &self,
        topic: impl Into<ChatId>,
        edit_hash: Hash,
        message: impl Into<String>,
    ) -> anyhow::Result<Header> {
        let header = self
            .publish(
                topic.into(),
                Payload::Chat(ChatPayload::EditMessage {
                    message: message.into(),
                    edit_hash,
                }),
                None,
            )
            .await?;

        Ok(header)
    }

    /// Collect all chat operations in a topic, reduced to the fields edit
    /// validation needs, keyed by operation hash.
    //
    // TODO: performance: this triggers a full log traversal on processing every
    // edit operation. We can improve this by building up the parallel ChatOp list
    // reduced state as part of the ACID refactors.
    pub(crate) async fn chat_ops(
        &self,
        topic: ChatId,
    ) -> anyhow::Result<std::collections::HashMap<Hash, ChatOp>> {
        let log_id = LogId::from(topic);
        let authors = self.op_store.get_authors(log_id).await?;

        let mut ops = std::collections::HashMap::new();
        for author in authors {
            for op in self.op_store.get_log(&author, &log_id, None).await? {
                let Some(body) = op.body.as_ref() else {
                    continue;
                };
                let Ok(Payload::Chat(chat)) = Payload::try_from_body(body) else {
                    continue;
                };
                let kind = match chat {
                    ChatPayload::Message(_) => ChatOpKind::Message,
                    ChatPayload::EditMessage { edit_hash, .. } => ChatOpKind::Edit(edit_hash),
                    _ => ChatOpKind::Other,
                };
                ops.insert(
                    op.header.hash(),
                    ChatOp {
                        author: DeviceId::from(op.header.verifying_key),
                        timestamp: op.header.timestamp.into(),
                        kind,
                    },
                );
            }
        }

        Ok(ops)
    }

    /// Return every edit operation in the topic that passes validation — i.e.
    /// the edits a receiving node would honor rather than ignore. Mirrors the
    /// rule applied in `process_app`, exposed for tests.
    #[cfg(any(test, feature = "testing"))]
    pub async fn valid_edits(
        &self,
        topic: impl Into<ChatId>,
    ) -> anyhow::Result<Vec<crate::chat::ValidEdit>> {
        let topic = topic.into();
        let ops = self.chat_ops(topic).await?;
        let log_id = LogId::from(topic);
        let authors = self.op_store.get_authors(log_id).await?;

        let mut edits = Vec::new();
        for author in authors {
            for op in self.op_store.get_log(&author, &log_id, None).await? {
                let Some(body) = op.body.as_ref() else {
                    continue;
                };
                let Ok(Payload::Chat(ChatPayload::EditMessage { message, edit_hash })) =
                    Payload::try_from_body(body)
                else {
                    continue;
                };
                let op_hash = op.header.hash();
                let editor = DeviceId::from(op.header.verifying_key);
                let timestamp: u64 = op.header.timestamp.into();
                if validate_edit(&ops, &edit_hash, editor, timestamp, Some(&op_hash)).is_ok() {
                    edits.push(crate::chat::ValidEdit {
                        op_hash,
                        target: edit_hash,
                        text: message,
                    });
                }
            }
        }

        Ok(edits)
    }

    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all, fields(me = ?self.device_id().aliased())))]
    pub async fn set_group_info(
        &self,
        chat_id: ChatId,
        info: crate::GroupInfo,
    ) -> anyhow::Result<Header> {
        let header = self
            .publish(chat_id, Payload::Chat(ChatPayload::GroupInfo(info)), None)
            .await?;

        Ok(header)
    }

    /// Returns the most recent `GroupInfo` payload in this topic's logs, or
    /// `None` if no member has authored one yet. "Most recent" is
    /// `(timestamp, seq_num)` across all author logs — matches the resolution
    /// in `GroupChatStore.info` on the frontend.
    pub async fn get_group_info(
        &self,
        topic_id: TopicId,
    ) -> anyhow::Result<Option<crate::GroupInfo>> {
        let log_id = LogId::from_topic(topic_id);
        let authors = self.op_store.get_authors(log_id).await?;
        let mut latest: Option<(Header, crate::GroupInfo)> = None;
        for author in authors {
            for op in self.op_store.get_log(&author, &log_id, None).await? {
                let Some(body) = op.body else { continue };
                let Ok(Payload::Chat(ChatPayload::GroupInfo(info))) = Payload::try_from_body(&body)
                else {
                    continue;
                };
                let is_later = match &latest {
                    None => true,
                    Some((h, _)) => {
                        op.header.timestamp > h.timestamp
                            || (op.header.timestamp == h.timestamp && op.header.seq_num > h.seq_num)
                    }
                };
                if is_later {
                    latest = Some((op.header, info));
                }
            }
        }
        Ok(latest.map(|(_, d)| d))
    }

    /// Tombstone an operation: record its hash in the topic's persisted
    /// tombstone set so its payload is never stored or synced again, and
    /// immediately drop any payload already stored for it.
    ///
    /// This has the effect that when the operation is played back, it will
    /// not have a payload. Therefore, payloads for which [`Self::is_tombstoneable`]
    /// is `true` should not cause state changes when processed!
    pub async fn tombstone_operation(
        &self,
        topic: TopicId,
        operation: &Operation,
    ) -> anyhow::Result<()> {
        let Some(payload) = Payload::try_from_body_opt(operation.body.as_ref())? else {
            return Ok(());
        };
        if self.is_tombstoneable(&payload) {
            let hash = operation.hash;
            self.unprocess_app(operation).await?;
            self.op_store.delete_body(&hash).await?;
            self.local_store.add_tombstone(topic, hash).await?;
        } else {
            tracing::warn!(operation = ?operation.hash.aliased(), "operation is not tombstoneable");
        }
        Ok(())
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

        if let Some(handle) = self.blob_fetch_handle.lock().await.take() {
            handle.abort();
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

    pub async fn store_media(
        &self,
        topic: TopicId,
        media: OutgoingMedia,
    ) -> anyhow::Result<MediaBundle> {
        let mut items = vec![];
        match media {
            OutgoingMedia::Photos { photos } => {
                for photo in photos {
                    let size = photo.data.len() as u64;
                    ensure_blob_size(size, &photo.name)?;
                    let hash = self.blob_sync.store_blob(topic, photo.data).await?;
                    items.push(MediaMetadata {
                        name: photo.name,
                        mime_type: photo.mime_type,
                        size,
                        hash,
                        kind: MediaMetaKind::Photo,
                    });
                }
            }
            OutgoingMedia::File { file } => {
                let size = file.data.len() as u64;
                ensure_blob_size(size, &file.name)?;
                let hash = self.blob_sync.store_blob(topic, file.data).await?;
                items.push(MediaMetadata {
                    name: file.name,
                    mime_type: file.mime_type,
                    size,
                    hash,
                    kind: MediaMetaKind::File,
                });
            }
        }
        Ok(MediaBundle::from(items))
    }

    /// Load the raw bytes of a single blob by its hash from the local blob store.
    ///
    /// With `timeout: Some(d)` the call triggers an immediate on-demand download
    /// (rather than waiting for the background fetch loop's next pass, which can
    /// be up to a minute away) and polls the local store until the blob is
    /// present or `d` elapses. This is what makes a user-driven retry actually
    /// re-attempt the fetch. `None` reads once and errors immediately if the
    /// blob is absent.
    ///
    /// Used by the `irohblob://` URI scheme handler to serve media to the webview.
    pub async fn load_blob(
        &self,
        hash: &str,
        timeout: Option<std::time::Duration>,
    ) -> anyhow::Result<Vec<u8>> {
        let hash: iroh_blobs::Hash = hash.parse()?;
        let Some(timeout) = timeout else {
            return Ok(self.blob_sync.blobs.get_bytes(hash).await?.to_vec());
        };

        if !self.blob_sync.blobs.has(hash).await.unwrap_or(false) {
            // Kick the download in the background so the poll below can still
            // observe the blob arriving via any path (this fetch, the
            // background loop, or a mailbox relay) rather than blocking on one.
            let blob_sync = self.blob_sync.clone();
            tokio::spawn(async move {
                blob_sync.fetch_now(hash, timeout).await;
            });
        }

        let deadline = std::time::Instant::now() + timeout;
        loop {
            if self.blob_sync.blobs.has(hash).await.unwrap_or(false) {
                return Ok(self.blob_sync.blobs.get_bytes(hash).await?.to_vec());
            }
            if std::time::Instant::now() >= deadline {
                anyhow::bail!("blob {hash} not available after {timeout:?}");
            }
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        }
    }

    pub async fn load_media(&self, meta: Vec<MediaMetadata>) -> anyhow::Result<OutgoingMedia> {
        let mut items = vec![];
        for item in meta {
            let data = self
                .blob_sync
                .blobs
                .get_bytes(item.hash)
                .await
                .context(format!("failed to load blob: {item:?}"))?;
            items.push((item, data));
        }

        let (photos, mut other): (Vec<_>, Vec<_>) = items
            .into_iter()
            .partition(|(item, _)| item.kind == MediaMetaKind::Photo);

        if other.len() > 1 {
            return Err(anyhow::anyhow!(
                "multiple files are not supported. photos: {photos:?}, other: {other:?}",
            ));
        } else if photos.len() >= 1 && other.len() == 1 {
            return Err(anyhow::anyhow!(
                "photos and other media in the same message are not supported. photos: {photos:?}, other: {other:?}",
            ));
        } else if other.len() == 1 {
            let (item, data) = other.pop().unwrap();
            return Ok(OutgoingMedia::File {
                file: OutgoingFile {
                    data: data.to_vec(),
                    name: item.name,
                    mime_type: item.mime_type,
                },
            });
        } else {
            let photos = photos
                .into_iter()
                .map(|(item, data)| crate::chat::OutgoingPhoto {
                    data: data.to_vec(),
                    name: item.name,
                    mime_type: item.mime_type,
                })
                .collect();
            return Ok(OutgoingMedia::Photos { photos });
        }
    }
}

#[cfg(test)]
mod blob_load_tests {
    use crate::NodeConfig;
    use crate::testing::TestNode;
    use std::time::Duration;

    #[tokio::test(flavor = "multi_thread")]
    async fn load_blob_present_returns_bytes_without_timeout() {
        let node = TestNode::new(NodeConfig::testing(), "alice").await;
        let tag = node.blobs().add_bytes(b"hello".to_vec()).await.unwrap();
        let hash = tag.hash.to_string();

        let got = node.load_blob(&hash, None).await.unwrap();
        assert_eq!(got, b"hello");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn load_blob_missing_without_timeout_errors_immediately() {
        let node = TestNode::new(NodeConfig::testing(), "alice").await;
        let missing = iroh_blobs::Hash::new(b"missing-without-timeout").to_string();

        let err = node.load_blob(&missing, None).await;
        assert!(err.is_err());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn load_blob_missing_with_timeout_errors_after_deadline() {
        let node = TestNode::new(NodeConfig::testing(), "alice").await;
        let missing = iroh_blobs::Hash::new(b"missing-with-timeout").to_string();

        let start = std::time::Instant::now();
        let err = node
            .load_blob(&missing, Some(Duration::from_millis(400)))
            .await;
        assert!(err.is_err());
        assert!(start.elapsed() >= Duration::from_millis(400));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn load_blob_with_timeout_returns_blob_that_lands_mid_wait() {
        let node = TestNode::new(NodeConfig::testing(), "alice").await;
        let content = b"arrives-late".to_vec();
        let hash = iroh_blobs::Hash::new(&content);

        let blobs = node.blobs();
        let content2 = content.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(300)).await;
            blobs.add_bytes(content2).await.unwrap();
        });

        let got = node
            .load_blob(&hash.to_string(), Some(Duration::from_secs(3)))
            .await
            .unwrap();
        assert_eq!(got, content);
    }
}
