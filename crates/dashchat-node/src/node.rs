pub(crate) mod actor;
mod app_processing;
mod message_acks;
pub(crate) mod publish;
mod report;

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use crate::blob_sync::{BlobFetchConfig, BlobFetchPool, BlobSync, SENTINEL_OP_HASH};
use crate::compat::Capabilities;
use crate::error::{
    AddContactError, AddContactResult, Error, RemoveGroupMemberError, ShutdownError,
};
use crate::filesystem::Filesystem;
use crate::node::actor::{Actor, Command};
#[cfg(feature = "testing")]
use crate::testing::TestNode;
use aliased::Aliasing;
use anyhow::{Context, Result};
use chrono::{Duration, Utc};
use dashchat_utils::blob_sync::MAX_BLOB_BYTES;
use p2panda::network::MdnsDiscoveryMode;
use p2panda::operation::{Header, LogId, Operation};
use p2panda::{Hash, NetworkId, Node as P2PandaNode, NodeId, RelayUrl, VerifyingKey};
use p2panda_auth::Access;
use p2panda_auth::group::resolver::StrongRemove;
use p2panda_auth::group::{GroupAction, GroupMember};
use p2panda_net::discovery::DiscoveryConfig;
use p2panda_spaces::ActorId;
use tokio::sync::{Mutex, mpsc, oneshot};

use mailbox_client::manager::{Mailboxes, MailboxesConfig};
use tokio::task::JoinHandle;

use crate::chat::{
    ChatMessageContent, ChatOp, ChatOpKind, EditCandidate, ReplyCandidate, ValidChatOps,
    collect_deletable_edit_chain, resolve_message_root,
};
use crate::contact::{AddContactQrCode, InboxTopic};
use crate::mailbox::MailboxOperation;
use crate::payload::{AnnouncementsPayload, ChatPayload, InboxPayload, Payload, Profile};
use crate::stores::{GroupStore, LocalStore, NodeKeys, OpProjection, OpStore};
use crate::topic::{Topic, TopicId, kind};
use crate::{
    AgentId, AsBody, ChatId, ChatReaction, DeleteCandidate, DeleteMessageError, DeviceGroupId,
    DeviceGroupPayload, DeviceId, DirectChatId, EditMessageError, FakeAgentId, MediaBundle,
    MediaMetadata, OutgoingFile, OutgoingMedia, SendMessageError,
};
use dashchat_utils::{NETWORK_ID, RELAY_URL};

pub use app_processing::{Notification, OpNotification, SystemNotification};

#[derive(Clone, Debug)]
pub struct NodeConfig {
    pub contact_code_expiry: Duration,
    pub mailboxes_config: MailboxesConfig,
    pub capabilities: Capabilities,
    pub network_id: NetworkId,
    pub mdns_mode: MdnsDiscoveryMode,
    /// Whether to register the node with the hardcoded relay so it is reachable
    /// over the internet.
    pub use_relay: bool,
    /// Whether to participate in peer-to-peer connectivity with other Dash Chat
    /// clients. When disabled, all communication flows through mailbox servers.
    ///
    /// This flag itself gates two of the four p2p surfaces:
    /// - **p2panda random-walk discovery**: when false, zero random walkers run
    ///   (see [`Node::init`]), so the node never gossips its own transport info
    ///   to peers nor learns theirs.
    /// - **p2panda bootstrap nodes**: when false, peers are never registered as
    ///   bootstrap nodes (see `register_bootstrap_node`), so discovery can't
    ///   reach them directly over a relay.
    ///
    /// The other two surfaces are also controlled by sibling fields:
    /// - **mDNS discovery**: [`Self::mdns_mode`].
    /// - **iroh relay** (internet reachability / NAT traversal): [`Self::use_relay`].
    ///
    /// [`Self::no_p2p`] disables all four together.
    /// The Node's initialization will reject any config with `enable_p2p` set to false
    /// and either `mdns_mode` or `use_relay` set to active/true.
    ///
    /// The iroh endpoint itself
    /// always stays up — mailbox blob/media exchange rides it and is unaffected.
    /// (The blob fetcher does still *attempt* a direct dial to a blob's author
    /// as a fallback source, but with every discovery surface off it has no
    /// address to dial, so those attempts cannot connect.)
    pub enable_p2p: bool,
    /// Whether to open the blob store and run blob sync. When disabled, all
    /// media send/fetch/serve operations error. Independent of [`Self::enable_p2p`]:
    /// `no_p2p` nodes still exchange media blobs through mailboxes over the iroh
    /// endpoint. Only the iOS push extension disables this — it never touches
    /// media, and opening the iroh-blobs `redb` metadata store would deadlock on
    /// the exclusive single-process lock the always-on main app already holds.
    pub enable_blob_sync: bool,
    pub blob_fetch: BlobFetchConfig,
    /// How often the followup task re-announces still-unfetched blob hashes to
    /// their mailboxes.
    pub unfetched_blob_followup_interval: std::time::Duration,
    /// How long the delivery-ack writer waits after new operations arrive
    /// before publishing a [`ChatPayload::MessageAck`], so a burst of incoming
    /// operations is covered by a single ack.
    pub message_ack_debounce: std::time::Duration,
    /// Whether to publish delivery acks at all. Only the iOS push extension
    /// disables this — its short-lived background node must not author
    /// operations.
    pub enable_message_acks: bool,
}

impl NodeConfig {
    /// Disable p2p features and only use mailbox-based communication.
    pub fn no_p2p(mut self) -> Self {
        self.mdns_mode = MdnsDiscoveryMode::Disabled;
        self.use_relay = false;
        self.enable_p2p = false;
        self
    }

    /// Skip opening the blob store entirely; media operations become unavailable.
    pub fn no_blob_sync(mut self) -> Self {
        self.enable_blob_sync = false;
        self
    }

    #[cfg(feature = "testing")]
    pub fn testing() -> Self {
        use crate::compat::Capabilities;

        let mut mailboxes_config = MailboxesConfig::default();
        mailboxes_config.active_interval = std::time::Duration::from_millis(500);
        mailboxes_config.degraded_interval = std::time::Duration::from_millis(1000);
        mailboxes_config.stopped_interval = std::time::Duration::from_millis(1500);
        mailboxes_config.between_polls_delay = std::time::Duration::from_millis(100);
        Self {
            contact_code_expiry: Duration::days(7),
            mailboxes_config,
            capabilities: Capabilities::current(),
            network_id: *NETWORK_ID,
            // In testing we disable mDNS discovery and do not provide a relay address so as not
            // to effect expected behavior of existing tests.
            mdns_mode: MdnsDiscoveryMode::Disabled,
            use_relay: false,
            enable_p2p: true,
            enable_blob_sync: true,
            // Retry blob downloads quickly so tests don't wait on the
            // production-scale pass interval.
            blob_fetch: BlobFetchConfig {
                concurrency: 4,
                pass_interval: std::time::Duration::from_secs(1),
                attempt_timeout: std::time::Duration::from_secs(3),
                retry_cooldown: std::time::Duration::from_secs(1),
            },
            unfetched_blob_followup_interval: std::time::Duration::from_secs(1),
            message_ack_debounce: std::time::Duration::from_millis(300),
            enable_message_acks: true,
        }
    }

    #[cfg(feature = "testing")]
    pub fn random_network_id(mut self) -> Self {
        self.network_id = p2panda::Topic::random().into();
        self
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
            use_relay: true,
            enable_p2p: true,
            enable_blob_sync: true,
            blob_fetch: BlobFetchConfig::default(),
            unfetched_blob_followup_interval: std::time::Duration::from_secs(60),
            message_ack_debounce: std::time::Duration::from_secs(3),
            enable_message_acks: true,
        }
    }
}

pub type DashResolver = StrongRemove<VerifyingKey, Hash, Operation, ()>;

#[derive(Clone)]
pub struct Node {
    pub op_store: OpStore,

    pub mailboxes: Mailboxes<MailboxOperation, OpStore>,

    pub config: NodeConfig,

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
    pub projection: OpProjection,
    group_store: GroupStore,
    node_keys: NodeKeys,

    filesystem: Filesystem,
    /// `None` when [`NodeConfig::enable_blob_sync`] is off (the iOS push
    /// extension), which only reads the operation to build a notification and
    /// never touches media blobs. Skipping it avoids opening the iroh-blobs
    /// `redb` metadata store — whose exclusive single-process lock the always-on
    /// main app holds, which would otherwise deadlock the extension's node build.
    blob_sync: Option<BlobSync>,
    blob_fetch_handle: Arc<Mutex<Option<JoinHandle<()>>>>,
    endpoint: p2panda::Endpoint,
    network_change_handle: Arc<Mutex<Option<JoinHandle<()>>>>,
    unfetched_blob_trigger: Arc<tokio::sync::Notify>,
    unfetched_blob_followup_handle: Arc<Mutex<Option<JoinHandle<()>>>>,
    message_ack_trigger: Arc<tokio::sync::Notify>,
    message_ack_handle: Arc<Mutex<Option<JoinHandle<()>>>>,
    dirty_ack_topics: Arc<std::sync::Mutex<HashSet<ChatId>>>,
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
        let pool = crate::stores::create_sqlite_pool(filesystem.local_store_path()).await?;
        let local_store = LocalStore::new(pool.clone()).await?;
        let projection = OpProjection::new(pool.clone()).await?;
        let node_keys = local_store.node_keys().await?;

        Self::init(
            filesystem,
            local_store,
            projection,
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
        projection: OpProjection,
        node_keys: NodeKeys,
        config: NodeConfig,
        notification_tx: Option<mpsc::Sender<Notification>>,
        topic_subscribed_tx: Option<mpsc::Sender<TopicId>>,
    ) -> Result<Self> {
        // A no-p2p config must fully disable p2p: `enable_p2p == false` is only
        // coherent when mDNS is disabled and no relay is used. Reject a config
        // that contradicts itself rather than silently leaving a p2p surface on.
        if !config.enable_p2p
            && (!matches!(config.mdns_mode, MdnsDiscoveryMode::Disabled) || config.use_relay)
        {
            anyhow::bail!(
                "invalid NodeConfig: enable_p2p is false but a p2p surface is still on \
                 (mdns_mode = {:?}, use_relay = {})",
                config.mdns_mode,
                config.use_relay,
            );
        }

        // === p2panda node === //

        let url = format!("sqlite://{}", filesystem.op_store_path().to_string_lossy());
        let mut builder = P2PandaNode::builder()
            .network_id(config.network_id)
            .signing_key(node_keys.private_key.clone())
            .database_url(&url)
            .mdns_mode(config.mdns_mode.clone())
            // Acknowledge operations explicitly, only once application-layer
            // processing has finished (see `spawn_application_processor_task`).
            .ack_policy(p2panda::node::AckPolicy::Explicit);

        if config.use_relay {
            builder = builder.relay_url(RELAY_URL.clone());
        }

        // With p2p disabled, run zero random-walk discovery walkers so the node
        // never initiates discovery sessions. Otherwise, inserting a mailbox's
        // address (a full p2panda node when run in-process) would let discovery
        // gossip our transport info through it, leaking a direct path to peers.
        if !config.enable_p2p {
            builder = builder.discovery_config(DiscoveryConfig {
                random_walkers_count: 0,
                ..Default::default()
            });
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

        // The push extension never touches media and must not open the
        // iroh-blobs store — its `redb` metadata db takes an exclusive
        // single-process lock the always-on main app already holds, which would
        // deadlock this build. It reads the operation and builds a notification
        // from its payload only, so blob sync is skipped entirely.
        let blob_sync = if config.enable_blob_sync {
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
            Some(
                BlobSync::new(
                    endpoint.clone(),
                    filesystem.blobs_store_path(),
                    blob_fetch,
                    source_lookup,
                    local_store.clone(),
                )
                .await?,
            )
        } else {
            None
        };

        // === node === //

        let (processor_cancel_tx, processor_cancel_rx) = mpsc::channel(1);
        let node = Self {
            op_store,
            mailboxes,
            config,
            filesystem,
            local_store,
            projection,
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
            endpoint,
            network_change_handle: Default::default(),
            unfetched_blob_trigger: Default::default(),
            unfetched_blob_followup_handle: Default::default(),
            message_ack_trigger: Default::default(),
            message_ack_handle: Default::default(),
            dirty_ack_topics: Default::default(),
        };

        // === application processor task === //

        let processor_handle =
            node.spawn_application_processor_task(events_rx, processor_cancel_rx);
        node.processor_handle.lock().await.replace(processor_handle);

        // === blob fetch loop === //

        if let Some(blob_sync) = &node.blob_sync {
            let blob_fetch_handle = blob_sync.spawn_fetch_loop(node.config.blob_fetch.clone());
            node.blob_fetch_handle
                .lock()
                .await
                .replace(blob_fetch_handle);
        }

        // === network change notifier === //

        let network_change_handle =
            crate::network_change_notifier::spawn(node.endpoint.clone(), node.mailboxes.clone());
        node.network_change_handle
            .lock()
            .await
            .replace(network_change_handle);

        // === unfetched blob followup loop === //

        let followup_handle = crate::spawn_unfetched_blob_followup_task(
            node.clone(),
            node.config.unfetched_blob_followup_interval,
            node.unfetched_blob_trigger.clone(),
        );
        node.unfetched_blob_followup_handle
            .lock()
            .await
            .replace(followup_handle);

        // === message ack writer === //

        if node.config.enable_message_acks {
            let ack_handle = message_acks::spawn_message_ack_task(
                node.clone(),
                node.config.message_ack_debounce,
                node.message_ack_trigger.clone(),
            );
            node.message_ack_handle.lock().await.replace(ack_handle);
        }

        // === topics === //

        node.initialize_stored_topics().await?;

        Ok(node)
    }

    pub fn data_path(&self) -> &PathBuf {
        self.filesystem.data_path()
    }

    pub async fn get_active_inbox_topics(&self) -> Result<BTreeSet<InboxTopic>, Error> {
        self.local_store
            .get_advertised_inbox_topics()
            .await
            .map_err(|err| Error::GetActiveInboxes(format!("{err}")))
    }

    /// Create a new contact QR code with configured expiry time,
    /// subscribe to the inbox topic for it, and register the topic as active.
    pub async fn create_add_contact_qr_code(&self) -> Result<AddContactQrCode, crate::Error> {
        let (inbox_topic, nonce) = InboxTopic::new_random(
            &self.device_id(),
            Utc::now() + self.config.contact_code_expiry,
        );
        self.initialize_topic(*inbox_topic.topic)
            .await
            .map_err(|err| crate::Error::InitializeTopic(format!("{err}")))?;
        self.local_store
            .add_active_inbox_topic(inbox_topic.clone())
            .await
            .map_err(|err| crate::Error::AddActiveInbox(format!("{err}")))?;

        let profile_name = self
            .my_profile()
            .await
            .ok()
            .flatten()
            .map(|profile| profile.full_name())
            .unwrap_or_default();

        Ok(AddContactQrCode::new(self.device_id(), nonce, profile_name))
    }

    pub fn agent_id(&self) -> AgentId {
        self.node_keys.agent_id
    }

    pub fn device_id(&self) -> DeviceId {
        self.node_keys.device_id()
    }

    pub fn fake_agent_id(&self) -> FakeAgentId {
        self.device_id().into()
    }

    pub fn blob_sync_optional(&self) -> Option<&crate::blob_sync::BlobSync> {
        self.blob_sync.as_ref()
    }

    #[cfg(feature = "testing")]
    pub fn blob_sync(&self) -> &crate::blob_sync::BlobSync {
        self.blob_sync
            .as_ref()
            .expect("blob sync is enabled for p2p (testing) nodes")
    }

    pub fn unfetched_blob_tracker(
        &self,
    ) -> std::sync::Arc<dyn mailbox_client::UnfetchedBlobTracker> {
        crate::LocalStoreBlobTracker::new(self.local_store.clone())
    }

    /// A blob-bytes source backed by this node's blob store, for the toy mailbox
    /// client to upload blob bytes inline. Reads error when blob sync is disabled
    /// (e.g. the push extension), which makes the client fall back to announcing
    /// hashes only.
    pub fn blob_reader(&self) -> std::sync::Arc<dyn mailbox_client::BlobReader> {
        std::sync::Arc::new(NodeBlobReader {
            blob_sync: self.blob_sync.clone(),
        })
    }

    /// Wake the unfetched-blob followup task to run a reconciliation pass now
    /// (e.g. on unpause / network change).
    pub fn notify_unfetched_blob_followup(&self) {
        self.unfetched_blob_trigger.notify_one();
    }

    /// Use the node's device ID as an iroh endpoint id.
    ///
    /// This is equivalent to `self.iroh_endpoint().await.unwrap().id()`,
    /// but avoids the async call.
    ///
    /// PANICS if the device ID is not a valid verifying key.
    pub fn endpoint_id(&self) -> iroh::EndpointId {
        iroh::EndpointId::from_bytes(self.device_id().as_bytes())
            .expect("device id is a valid endpoint id")
    }

    /// The underlying iroh endpoint. An in-process mailbox shares this so its
    /// `/health` response advertises the node's dialing address.
    pub async fn iroh_endpoint(&self) -> Result<iroh::Endpoint> {
        Ok(self.endpoint.endpoint().await?)
    }

    /// Tear down all p2p connectivity by closing the iroh endpoint. Afterwards
    /// the node can neither dial nor accept peer connections, so no gossip sync
    /// happens; local reads and writes against the op store keep working. This
    /// is a one-way switch intended only for e2e tests that must observe
    /// pre-sync UI state without racing a direct p2p connection between two
    /// agents running on the same machine.
    #[cfg(feature = "testing")]
    pub async fn close_iroh_endpoint(&self) -> Result<()> {
        self.endpoint.endpoint().await?.close().await;
        Ok(())
    }

    /// Add (or refresh) a peer's dialing address (relay + direct addresses) in
    /// the p2panda address book so the iroh blob downloader can reach that peer
    /// by its EndpointId. Used for mailbox `/health` self-addresses and for peer
    /// addresses forwarded by a user's opt-in local mailbox. Always overwrites
    /// any existing entry so a stale one (refused by `AddressBookDiscovery`) is
    /// refreshed and becomes dialable again.
    pub async fn insert_peer_addr(&self, addr: iroh::EndpointAddr) -> Result<()> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.actor_tx
            .send(Command::RegisterPeerAddr { addr, reply_tx })
            .await
            .map_err(|err| anyhow::anyhow!("send to actor error: {err}"))?;
        reply_rx.await??;
        Ok(())
    }

    /// The node's blob sync, or an error when blob sync is disabled (the push
    /// extension never opens a blob store). Only the media send/serve paths —
    /// never reached by the extension — call this.
    fn require_blob_sync(&self) -> Result<&BlobSync> {
        self.blob_sync
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("media blob operations require blob sync enabled"))
    }

    #[cfg(feature = "testing")]
    /// The node's iroh-blobs protocol handle, sharing its blob store. An
    /// in-process mailbox uses this so relayed blobs land in—and are served
    /// from—the same store on the same endpoint as the node.
    pub fn blobs(&self) -> iroh_blobs::BlobsProtocol {
        self.blob_sync
            .as_ref()
            .expect("blob sync is enabled for p2p (testing) nodes")
            .blobs
            .clone()
    }

    #[cfg(feature = "testing")]
    /// The node's blob downloader, for an in-process mailbox to fetch blobs
    /// into the shared store over the node's endpoint.
    pub fn blob_downloader(&self) -> iroh_blobs::api::downloader::Downloader {
        self.blob_sync
            .as_ref()
            .expect("blob sync is enabled for p2p (testing) nodes")
            .downloader()
    }

    #[cfg(feature = "testing")]
    /// Topics the blob fetch pool currently associates with `hash`. Lets a test
    /// assert that startup hydration re-queued a stored op's blob.
    pub async fn blob_fetch_pool_topics_for(&self, hash: iroh_blobs::Hash) -> Vec<TopicId> {
        self.blob_sync
            .as_ref()
            .expect("blob sync is enabled for p2p (testing) nodes")
            .fetch_pool
            .topics_for(hash)
            .await
    }

    pub fn device_group_topic(&self) -> DeviceGroupId {
        Topic::device_group(self.agent_id()).into()
    }

    /// Get the topic for a direct chat between two public keys.
    ///
    /// The topic is the hashed sorted public keys.
    /// Anyone who knows the two public keys can derive the same topic.
    // TODO: is this a problem? Should we use a random topic instead?
    pub fn direct_chat_topic(&self, other: FakeAgentId) -> DirectChatId {
        let me = self.fake_agent_id();
        // TODO: use two secrets from each party to construct the topic
        let topic = Topic::direct_chat([me, other]);
        if me > other {
            topic.alias_named(&format!("direct({:?},{:?})", other.aliased(), me.aliased()))
        } else {
            topic.alias_named(&format!("direct({:?},{:?})", me.aliased(), other.aliased()))
        }
    }

    #[cfg(feature = "testing")]
    pub fn direct_chat_with(&self, other: &TestNode) -> DirectChatId {
        let other = other.fake_agent_id();
        self.direct_chat_topic(other)
    }

    /// Create a new direct chat Space.
    /// Note that only one node should create the space!
    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all, fields(me = ?self.device_id().aliased())))]
    pub async fn create_direct_chat_space(&self, other: FakeAgentId) -> anyhow::Result<()> {
        let topic = self.direct_chat_topic(other);

        let my_actor = self.agent_id();
        self.register_topic(topic).await?;

        let other_device_id = DeviceId::from(other);

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

        let contacts = self.projection.lookup_contacts(&device_ids).await?;
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
        let device_id = self
            .projection
            .lookup_contact_by_agent_id(person)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Contact not found in lookup table"))?;
        let person = FakeAgentId::from(device_id);
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
            .projection
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
        Ok(())
    }

    pub async fn get_groups(&self) -> anyhow::Result<Vec<ChatId>> {
        self.projection.get_group_chat_ids().await
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
        self.projection.get_profile(self.agent_id()).await
    }

    pub async fn get_profile(&self, agent_id: AgentId) -> anyhow::Result<Option<Profile>> {
        self.projection.get_profile(agent_id).await
    }

    pub async fn lookup_contact(&self, device_id: DeviceId) -> anyhow::Result<Option<AgentId>> {
        self.projection.lookup_contact_by_device_id(device_id).await
    }

    pub async fn all_contact_agent_ids(&self) -> anyhow::Result<BTreeSet<AgentId>> {
        self.projection.all_contact_agent_ids().await
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

    /// Send a message to a chat, optionally with media and/or a previous message to reply to.
    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all, fields(me = ?self.device_id().aliased())))]
    pub async fn send_message(
        &self,
        topic: impl Into<ChatId>,
        message: impl Into<String>,
        media: Option<OutgoingMedia>,
        reply: Option<Hash>,
    ) -> Result<Header, SendMessageError> {
        let chat_id: ChatId = topic.into();

        if let Some(target) = reply {
            let valid_ops = self.valid_chat_ops(chat_id).await?;
            let now = u64::from(p2panda_core::Timestamp::now());
            ReplyCandidate {
                target,
                timestamp: now,
                self_hash: None,
            }
            .validate(&valid_ops)?;
        }

        let meta = if let Some(media) = media {
            Some(
                self.store_media(chat_id.into(), SENTINEL_OP_HASH, media)
                    .await?,
            )
        } else {
            None
        };
        let message = ChatMessageContent::new(message, meta.clone(), reply);
        let header = self.send_message_raw(chat_id, message).await?;
        if let Some(bundle) = meta {
            let topic_id: TopicId = chat_id.into();
            for item in bundle.iter() {
                if let Err(err) = self
                    .require_blob_sync()?
                    .retag_blob(topic_id, self.device_id(), header.hash(), item.hash())
                    .await
                {
                    tracing::warn!(?err, "failed to retag blob after operation creation");
                }
            }
        }
        Ok(header)
    }

    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all, fields(me = ?self.device_id().aliased())))]
    pub async fn send_message_raw(
        &self,
        topic: impl Into<ChatId>,
        message: ChatMessageContent,
    ) -> anyhow::Result<Header> {
        let topic = topic.into();

        let header = self
            .publish(topic, Payload::Chat(ChatPayload::Message(message)), None)
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
        let valid_ops = self.valid_chat_ops(topic).await?;
        let now = u64::from(p2panda_core::Timestamp::now());
        EditCandidate {
            target: edit_hash,
            editor: self.device_id(),
            timestamp: now,
            self_hash: None,
        }
        .validate(&valid_ops)?;

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

    /// Delete a previously-sent message for everyone in the chat.
    ///
    /// `target` must be the most recent edit of the message (or the message
    /// itself when unedited), authored by us, within the delete window, and not
    /// already deleted. The full edit chain is collected and published in a
    /// `DeleteMessage` payload; processing it tombstones every operation in the
    /// chain. See [`DeleteError`](crate::chat::DeleteError).
    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all, fields(me = ?self.device_id().aliased())))]
    pub async fn delete_message_for_everyone(
        &self,
        topic: impl Into<ChatId>,
        target: Hash,
    ) -> Result<Header, DeleteMessageError> {
        let topic = topic.into();
        let ops = self.valid_chat_ops(topic).await?;
        let hashes = collect_deletable_edit_chain(&ops, &target)?;
        let now = u64::from(p2panda_core::Timestamp::now());
        DeleteCandidate {
            hashes: hashes.clone(),
            deleter: self.device_id(),
            delete_timestamp: now,
            self_hash: None,
        }
        .validate(&ops)?;

        let header = self
            .publish(
                topic,
                Payload::Chat(ChatPayload::DeleteMessage { hashes }),
                None,
            )
            .await?;

        Ok(header)
    }

    /// Delete a previously-sent message only for my own device group.
    ///
    /// Unlike [`Self::delete_message_for_everyone`], which requires the tip of
    /// the edit chain, `target` here may be any operation in the chain — it is
    /// resolved back to the original message before publishing, so the whole
    /// chain is captured whichever version the caller names.
    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all, fields(me = ?self.device_id().aliased())))]
    pub async fn delete_message_for_me(
        &self,
        topic: impl Into<ChatId>,
        target: Hash,
    ) -> Result<Header, DeleteMessageError> {
        let chat_id = topic.into();
        let ops = self.valid_chat_ops(chat_id).await?;
        // Resolve to the original message when we can, but fall back to the raw
        // target when its body is gone (already deleted for everyone) or never
        // fetched — such an op isn't in `valid_chat_ops`, and delete-for-me should
        // still just remove it locally instead of erroring. Pruning guarantees
        // every edit in `ops` has its target, so resolution fails only when
        // `target` itself is absent, leaving nothing to walk back through.
        //
        // TODO: ACID: this is something to tighten up when revisiting tombstone logic.
        let message_hash = resolve_message_root(&ops, &target).unwrap_or(target);

        let header = self
            .publish(
                self.device_group_topic(),
                Payload::DeviceGroup(DeviceGroupPayload::DeleteForMe(
                    crate::payload::DeleteForMePayload {
                        chat_id,
                        message_hash,
                    },
                )),
                Some(&format!("delete_message_for_me({:?})", chat_id.aliased())),
            )
            .await?;

        Ok(header)
    }

    /// Every tombstone in a chat, paired with why it was tombstoned. The
    /// frontend uses this to drop delete-for-me messages (and their edits) from
    /// view while keeping the delete-for-everyone placeholders.
    pub async fn chat_tombstones(
        &self,
        chat_id: impl Into<ChatId>,
    ) -> anyhow::Result<HashMap<Hash, crate::stores::TombstoneReason>> {
        self.projection.tombstones(chat_id.into().into()).await
    }

    /// Publish a delete without validating it. For testing the receiving-side
    /// handling of invalid deletes, which the author-side validation would
    /// otherwise prevent from ever being created.
    #[cfg(any(test, feature = "testing"))]
    pub async fn delete_message_raw(
        &self,
        topic: impl Into<ChatId>,
        hashes: BTreeSet<Hash>,
    ) -> anyhow::Result<Header> {
        let header = self
            .publish(
                topic.into(),
                Payload::Chat(ChatPayload::DeleteMessage { hashes }),
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
    pub(crate) async fn valid_chat_ops(&self, topic: ChatId) -> anyhow::Result<ValidChatOps> {
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
                    ChatPayload::Message(m) => ChatOpKind::Message { reply: m.reply() },
                    ChatPayload::EditMessage { edit_hash, .. } => ChatOpKind::Edit(edit_hash),
                    ChatPayload::DeleteMessage { hashes } => ChatOpKind::Delete(hashes),
                    _ => ChatOpKind::Other,
                };
                ops.insert(
                    op.header.hash(),
                    ChatOp {
                        author: DeviceId::from(op.header.verifying_key),
                        timestamp: op.header.timestamp.into(),
                        seq_num: op.header.seq_num,
                        kind,
                    },
                );
            }
        }

        let mut ops = ValidChatOps::new(ops);
        ops.prune();
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
        let valid_ops = self.valid_chat_ops(topic).await?;
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
                if valid_ops.contains(&op_hash) {
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

    /// Return every message in the topic carrying a reply annotation that
    /// passes receiving-side validation — i.e. the replies an honest node
    /// would render as quotes rather than ignore. Mirrors the rule applied in
    /// `process_app`, exposed for tests.
    #[cfg(any(test, feature = "testing"))]
    pub async fn valid_replies(
        &self,
        topic: impl Into<ChatId>,
    ) -> anyhow::Result<Vec<crate::chat::ValidReply>> {
        let topic = topic.into();
        let valid_ops = self.valid_chat_ops(topic).await?;
        let log_id = LogId::from(topic);
        let authors = self.op_store.get_authors(log_id).await?;

        let mut replies = Vec::new();
        for author in authors {
            for op in self.op_store.get_log(&author, &log_id, None).await? {
                let Some(body) = op.body.as_ref() else {
                    continue;
                };
                let Ok(Payload::Chat(ChatPayload::Message(message))) = Payload::try_from_body(body)
                else {
                    continue;
                };
                let op_hash = op.header.hash();
                let Some(ChatOp {
                    kind:
                        ChatOpKind::Message {
                            reply: Some(target),
                        },
                    ..
                }) = valid_ops.get(&op_hash)
                else {
                    continue;
                };
                replies.push(crate::chat::ValidReply {
                    op_hash,
                    target: *target,
                    text: message.message().to_string(),
                });
            }
        }

        Ok(replies)
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

        if let Some(handle) = self.unfetched_blob_followup_handle.lock().await.take() {
            handle.abort();
        }

        if let Some(handle) = self.message_ack_handle.lock().await.take() {
            handle.abort();
        }

        if let Some(handle) = self.network_change_handle.lock().await.take() {
            handle.abort();
        }

        // Release every held file lock before returning, or iOS SIGKILLs the
        // suspended app (0xdead10cc). File locks first, slow best-effort endpoint
        // close last, each time-bounded so none outlasts the suspension window.

        // iroh-blobs redb store: a file lock like the SQLite pools; fetch loop
        // aborted above so nothing else is using it now. Absent when blob sync
        // is disabled (the push extension), which never opens it.
        if let Some(blob_sync) = &self.blob_sync {
            match tokio::time::timeout(
                std::time::Duration::from_secs(5),
                blob_sync.blobs.store().shutdown(),
            )
            .await
            {
                Ok(Ok(())) => {}
                Ok(Err(err)) => tracing::warn!("failed to shut down blob store: {err:?}"),
                Err(_) => tracing::warn!("timed out shutting down blob store"),
            }
        }

        // Close pools last.
        // SqlitePool clones share state, so this drains every connection; the
        // sync tracker owns a separate pool (mailbox_sync_tracker.db).
        self.mailboxes.sync_tracker().close().await;
        self.local_store.close().await;
        self.op_store.close().await;

        // Holds only sockets (no file lock), so it goes last. The node keeps its
        // own endpoint clone, so the actor drop above doesn't release it.
        match self.endpoint.endpoint().await {
            Ok(endpoint) => {
                if tokio::time::timeout(std::time::Duration::from_secs(3), endpoint.close())
                    .await
                    .is_err()
                {
                    tracing::warn!("timed out closing iroh endpoint");
                }
            }
            Err(err) => tracing::warn!("failed to resolve iroh endpoint for close: {err:?}"),
        }

        Ok(())
    }

    /// Register the shared, idempotent state for a contact identified by their
    /// device pubkey and agent id:
    /// - register the contact as a bootstrap peer,
    /// - subscribe to their announcements, and
    /// - subscribe to our direct-chat topic.
    ///
    /// Safe to call repeatedly, so both the initiating `add_contact` path and the
    /// inbox request/ack handlers can call it.
    pub(crate) async fn establish_contact(
        &self,
        device_id: DeviceId,
        agent_id: AgentId,
    ) -> Result<(), Error> {
        // Register the contact as a bootstrap so p2panda discovery can reach it
        // directly over the internet (relay + pkarr), rather than depending on a
        // mutually-reachable mailbox to introduce the two nodes.
        self.register_bootstrap_node(*device_id)
            .await
            .map_err(|e| Error::RegisterBootstrap(e.to_string()))?;
        // Subscribe to the contact's announcements to receive their group
        // control messages, and to our shared direct-chat topic.
        self.register_topic(Topic::announcements(agent_id))
            .await
            .map_err(|e| Error::InitializeTopic(e.to_string()))?;
        let fake_agent_id = FakeAgentId::from(device_id);
        self.register_topic(self.direct_chat_topic(fake_agent_id))
            .await
            .map_err(|e| Error::InitializeTopic(e.to_string()))?;
        Ok(())
    }

    /// Store someone as a contact, and:
    /// - register their spaces keybundle so we can add them to spaces
    /// - subscribe to their inbox
    /// - store them in the contacts map
    /// - send an invitation to them to do the same
    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all, fields(me = ?self.device_id().aliased())))]
    pub async fn add_contact(
        &self,
        contact: AddContactQrCode,
    ) -> Result<AddContactResult, AddContactError> {
        tracing::debug!(
            device_pub_key = ?contact.device_pubkey.aliased(),
            "adding contact",
        );

        if contact.device_pubkey == self.device_id() {
            return Err(AddContactError::CannotAddSelf);
        }

        let direct_chat_topic_id = self.direct_chat_topic(FakeAgentId::from(contact.device_pubkey));

        // If we already sent this device a contact request, don't publish a
        // duplicate request or pending marker. Return the existing direct-chat
        // topic id so the caller can navigate there.
        if self
            .has_outgoing_pending_request(contact.device_pubkey)
            .await
            .map_err(|e| Error::AuthorOperation(e.to_string()))?
        {
            return Ok(AddContactResult::AlreadyRequested(direct_chat_topic_id));
        }

        // Register the scanned contact as a bootstrap so p2panda discovery can
        // reach it directly over the internet (relay + pkarr), rather than
        // depending on a mutually-reachable mailbox to introduce the two nodes.
        self.register_bootstrap_node(*contact.device_pubkey)
            .await
            .map_err(|e| Error::RegisterBootstrap(e.to_string()))?;

        // Subscribe to the shared direct-chat topic right away so messages sent
        // before the owner accepts already sync in both directions.
        self.register_topic(direct_chat_topic_id)
            .await
            .map_err(|e| Error::InitializeTopic(e.to_string()))?;

        // SPACES: Register the member in the spaces manager

        let inbox_topic = InboxTopic::from_nonce(
            &contact.device_pubkey,
            &contact.inbox_nonce,
            Utc::now() + self.config.contact_code_expiry,
        );

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

        self.initialize_topic(*inbox_topic.topic)
            .await
            .map_err(|e| Error::InitializeTopic(e.to_string()))?;

        // Mint a private reply inbox for this exchange and listen on it for
        // the owner's ack. We do NOT persist the (possibly shared) advertised
        // inbox we scanned — only the owner keeps camping on that — so other
        // scanners of the same QR never share a return channel with us.
        let reply_inbox = InboxTopic {
            topic: Topic::inbox().alias_named(&format!(
                "reply_inbox({:?},peer={})",
                self.device_id().aliased(),
                &hex::encode(&contact.device_pubkey.as_bytes()[..4])
            )),
            expires_at: Utc::now() + self.config.contact_code_expiry,
        };

        self.initialize_topic(*reply_inbox.topic)
            .await
            .map_err(|e| Error::InitializeTopic(e.to_string()))?;

        self.local_store
            .add_reply_inbox_topic(reply_inbox.clone(), contact.device_pubkey)
            .await
            .map_err(|e| Error::AddActiveInbox(format!("{e}")))?;
        let Some(profile) = self
            .my_profile()
            .await
            .map_err(|e| Error::AuthorOperation(e.to_string()))?
        else {
            return Err(AddContactError::ProfileNotCreated);
        };

        self.publish(
            inbox_topic.topic,
            Payload::Inbox(InboxPayload::ContactRequest {
                profile,
                agent_id: self.agent_id(),
                reply_topic: reply_inbox.topic.clone(),
            }),
            Some(&format!(
                "add_contact/contact_request({:?})",
                contact.device_pubkey.aliased()
            )),
        )
        .await
        .map_err(|e| Error::AuthorOperation(e.to_string()))?;

        // Record a pending request in our own device group so the UI can show a
        // placeholder chat until the owner's ack arrives. Keyed on the owner's
        // device pubkey, since we don't know their agent id yet.
        self.publish(
            self.device_group_topic(),
            Payload::DeviceGroup(DeviceGroupPayload::PendingContactRequest {
                device_pubkey: contact.device_pubkey,
                profile_name: contact.profile_name,
                direct_chat_topic_id,
            }),
            Some(&format!(
                "add_contact/pending({:?})",
                contact.device_pubkey.aliased()
            )),
        )
        .await
        .map_err(|e| Error::AuthorOperation(e.to_string()))?;

        Ok(AddContactResult::NewRequest(direct_chat_topic_id))
    }

    /// Record a mutual contact by publishing the contact marker into our own
    /// device group. Contact establishment (mapping, topics, bootstrap) is done
    /// separately via [`Self::establish_contact`].
    pub(crate) async fn publish_add_contact(
        &self,
        agent_id: AgentId,
        direct_chat_topic_id: ChatId,
    ) -> Result<(), Error> {
        self.publish(
            self.device_group_topic(),
            Payload::DeviceGroup(DeviceGroupPayload::AddContact {
                agent_id,
                direct_chat_topic_id,
            }),
            Some(&format!("add_contact({:?})", agent_id.aliased())),
        )
        .await
        .map_err(|e| Error::AuthorOperation(e.to_string()))?;
        Ok(())
    }

    /// Accept a contact request that was received over our advertised inbox.
    /// On receipt we only recorded the requester's identity + profile locally;
    /// accepting now performs the network establishment we deliberately deferred
    /// — registering the requester as a bootstrap node and subscribing to their
    /// topics — replies with our profile (which also signals acceptance so the
    /// requester can complete their side), publishes the contact marker, and
    /// creates the shared direct-chat space.
    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all, fields(me = ?self.device_id().aliased())))]
    pub async fn accept_contact(&self, agent_id: AgentId) -> Result<(), AddContactError> {
        // The requester's device was mapped to their agent_id when the request
        // arrived; recover it to establish their network presence.
        let device_pubkey = self
            .projection
            .lookup_contact_by_agent_id(agent_id)
            .await
            .map_err(|e| Error::AuthorOperation(e.to_string()))?
            .ok_or_else(|| {
                AddContactError::CreateDirectChat(format!(
                    "no pending contact request found for agent {:?}",
                    agent_id.aliased()
                ))
            })?;
        self.establish_contact(device_pubkey, agent_id).await?;

        // Reply to the requester with our profile over their private reply
        // topic. This is the point at which we first disclose our profile and
        // signals that we accepted, letting them complete the exchange.
        if let Some(reply_topic) = self
            .find_contact_request_reply_topic(agent_id)
            .await
            .map_err(|e| Error::AuthorOperation(e.to_string()))?
        {
            self.reply_to_contact_request(reply_topic).await?;
        } else {
            tracing::warn!(
                agent_id = ?agent_id.aliased(),
                "accepted contact but found no request to reply to"
            );
        }

        let fake_agent_id = FakeAgentId::from(device_pubkey);
        self.publish_add_contact(agent_id, self.direct_chat_topic(fake_agent_id))
            .await?;

        self.create_direct_chat_space(fake_agent_id)
            .await
            .map_err(|e| AddContactError::CreateDirectChat(e.to_string()))?;

        // Messages the requester sent before acceptance were processed but
        // deliberately not acked; now that they are a contact, cover them.
        self.mark_ack_topic_dirty(self.direct_chat_topic(fake_agent_id));

        Ok(())
    }

    /// Returns true if we have an outgoing contact request recorded for
    /// `device_pubkey` (i.e. we scanned their code and are awaiting their ack).
    pub(crate) async fn has_outgoing_pending_request(
        &self,
        device_id: DeviceId,
    ) -> anyhow::Result<bool> {
        self.local_store
            .has_pending_reply_inbox_for(device_id)
            .await
    }

    /// Scan our advertised inbox logs for a pending [`InboxPayload::ContactRequest`]
    /// from `agent_id` and return its private reply topic, so [`Self::accept_contact`]
    /// can send our acceptance there. Returns `None` if no matching request is stored.
    async fn find_contact_request_reply_topic(
        &self,
        agent_id: AgentId,
    ) -> anyhow::Result<Option<Topic<kind::Inbox>>> {
        for inbox in self.local_store.get_advertised_inbox_topics().await? {
            let log_id = LogId::from_topic(*inbox.topic);
            for author in self.op_store.get_authors(log_id).await? {
                for op in self.op_store.get_log(&author, &log_id, None).await? {
                    let Some(body) = op.body else { continue };
                    let Ok(Payload::Inbox(InboxPayload::ContactRequest {
                        agent_id: req_agent,
                        reply_topic,
                        ..
                    })) = Payload::try_from_body(&body)
                    else {
                        continue;
                    };
                    if req_agent == agent_id {
                        return Ok(Some(reply_topic));
                    }
                }
            }
        }
        Ok(None)
    }

    /// Reply to an incoming contact request by sending our profile to the
    /// scanner's private reply topic, so the scanner learns it immediately over
    /// the inbox rather than waiting for announcements sync. We subscribe to the
    /// reply topic just long enough to publish to it.
    pub(crate) async fn reply_to_contact_request(
        &self,
        reply_topic: Topic<kind::Inbox>,
    ) -> Result<(), Error> {
        let Some(profile) = self
            .my_profile()
            .await
            .map_err(|e| Error::AuthorOperation(e.to_string()))?
        else {
            return Ok(());
        };
        self.initialize_topic(*reply_topic)
            .await
            .map_err(|e| Error::InitializeTopic(e.to_string()))?;
        self.publish(
            reply_topic,
            Payload::Inbox(InboxPayload::ContactRequestAccept {
                profile,
                agent_id: self.agent_id(),
            }),
            Some("reply_to_contact_request"),
        )
        .await
        .map_err(|e| Error::AuthorOperation(e.to_string()))?;
        Ok(())
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

    /// Block a contact. While blocked, all operations authored by the contact's
    /// devices are invalidated by the projection layer (except those needed to
    /// maintain group chats), so their messages never reach us.
    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all, fields(me = ?self.device_id().aliased())))]
    pub async fn block_contact(&self, agent_id: AgentId) -> Result<(), Error> {
        tracing::debug!("blocking contact: {:?}", agent_id.aliased());

        self.publish(
            self.device_group_topic(),
            Payload::DeviceGroup(DeviceGroupPayload::BlockAgent(agent_id)),
            Some(&format!("block_contact({:?})", agent_id.aliased())),
        )
        .await
        .map_err(|e| Error::AuthorOperation(e.to_string()))?;

        Ok(())
    }

    /// Unblock a previously blocked contact, allowing their operations to be
    /// processed again.
    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all, fields(me = ?self.device_id().aliased())))]
    pub async fn unblock_contact(&self, agent_id: AgentId) -> Result<(), Error> {
        tracing::debug!("unblocking contact: {:?}", agent_id.aliased());

        self.publish(
            self.device_group_topic(),
            Payload::DeviceGroup(DeviceGroupPayload::UnblockAgent(agent_id)),
            Some(&format!("unblock_contact({:?})", agent_id.aliased())),
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

        for topic in self.local_store.get_advertised_inbox_topics().await?.iter() {
            self.initialize_topic(
                *topic
                    .topic
                    .clone()
                    .alias_named(&format!("inbox({:?})", self.device_id().aliased())),
            )
            .await?;
        }

        for (topic, peer_device) in self
            .local_store
            .get_reply_inbox_topics_with_author()
            .await?
            .iter()
        {
            self.initialize_topic(*topic.topic.clone().alias_named(&format!(
                "reply_inbox({:?},peer={})",
                self.device_id().aliased(),
                &hex::encode(&peer_device.as_bytes()[..4])
            )))
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

    pub async fn store_media(
        &self,
        topic: TopicId,
        operation_hash: p2panda::Hash,
        media: OutgoingMedia,
    ) -> anyhow::Result<MediaBundle> {
        let mut items = vec![];
        match media {
            OutgoingMedia::Photos { photos } => {
                for photo in photos {
                    let size = photo.data.len() as u64;
                    ensure_blob_size(size, &photo.name)?;
                    let hash = self
                        .require_blob_sync()?
                        .store_blob(topic, self.device_id(), operation_hash, photo.data)
                        .await?;
                    items.push(MediaMetadata::Photo {
                        name: photo.name,
                        mime_type: photo.mime_type,
                        size,
                        width: photo.width,
                        height: photo.height,
                        hash,
                    });
                }
            }
            OutgoingMedia::File { file } => {
                let size = file.data.len() as u64;
                ensure_blob_size(size, &file.name)?;
                let hash = self
                    .require_blob_sync()?
                    .store_blob(topic, self.device_id(), operation_hash, file.data)
                    .await?;
                items.push(MediaMetadata::File {
                    name: file.name,
                    mime_type: file.mime_type,
                    size,
                    hash,
                });
            }
            OutgoingMedia::VoiceNote { voice_note } => {
                let size = voice_note.data.len() as u64;
                ensure_blob_size(size, "voice note")?;

                let hash = self
                    .require_blob_sync()?
                    .store_blob(topic, self.device_id(), operation_hash, voice_note.data)
                    .await?;
                items.push(MediaMetadata::VoiceNote {
                    mime_type: voice_note.mime_type,
                    size,
                    duration_ms: voice_note.duration_ms,
                    waveform: voice_note.waveform,
                    hash,
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
        let blob_sync = self.require_blob_sync()?;
        let Some(timeout) = timeout else {
            return Ok(blob_sync.blobs.get_bytes(hash).await?.to_vec());
        };

        if !blob_sync.blobs.has(hash).await.unwrap_or(false) {
            // Kick the download in the background so the poll below can still
            // observe the blob arriving via any path (this fetch, the
            // background loop, or a mailbox relay) rather than blocking on one.
            let blob_sync = blob_sync.clone();
            tokio::spawn(async move {
                blob_sync.fetch_now(hash, timeout).await;
            });
        }

        let deadline = std::time::Instant::now() + timeout;
        loop {
            if blob_sync.blobs.has(hash).await.unwrap_or(false) {
                return Ok(blob_sync.blobs.get_bytes(hash).await?.to_vec());
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
                .require_blob_sync()?
                .blobs
                .get_bytes(item.hash())
                .await
                .context(format!("failed to load blob: {item:?}"))?;
            items.push((item, data));
        }

        // A voice note or a file is always a single-item bundle; photos may be many.
        if items.len() == 1
            && matches!(
                items[0].0,
                MediaMetadata::VoiceNote { .. } | MediaMetadata::File { .. }
            )
        {
            let (item, data) = items.pop().unwrap();
            let outgoing_media = match item {
                MediaMetadata::VoiceNote {
                    mime_type,
                    duration_ms,
                    waveform,
                    ..
                } => Ok(OutgoingMedia::VoiceNote {
                    voice_note: crate::chat::OutgoingVoiceNote {
                        data: data.to_vec(),
                        mime_type,
                        duration_ms,
                        waveform,
                    },
                }),
                MediaMetadata::File {
                    name, mime_type, ..
                } => Ok(OutgoingMedia::File {
                    file: OutgoingFile {
                        data: data.to_vec(),
                        name,
                        mime_type,
                    },
                }),
                MediaMetadata::Photo { .. } => unreachable!(),
            };
            return outgoing_media;
        }

        let photos = items
            .into_iter()
            .map(|(item, data)| match item {
                MediaMetadata::Photo {
                    name,
                    mime_type,
                    width,
                    height,
                    ..
                } => Ok(crate::chat::OutgoingPhoto {
                    data: data.to_vec(),
                    name,
                    mime_type,
                    width,
                    height,
                }),
                other => Err(anyhow::anyhow!(
                    "unsupported media combination in a single message: {other:?}"
                )),
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        Ok(OutgoingMedia::Photos { photos })
    }
}

/// [`mailbox_client::BlobReader`] backed by the node's blob store.
struct NodeBlobReader {
    blob_sync: Option<BlobSync>,
}

#[async_trait::async_trait]
impl mailbox_client::BlobReader for NodeBlobReader {
    async fn read_blob(&self, hash: iroh_blobs::Hash) -> anyhow::Result<bytes::Bytes> {
        let blob_sync = self
            .blob_sync
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("blob sync disabled"))?;
        Ok(blob_sync.blobs.get_bytes(hash).await?)
    }
}

#[cfg(test)]
mod config_validation_tests {
    use crate::NodeConfig;
    use crate::node::Node;

    async fn init_result(config: NodeConfig) -> anyhow::Result<Node> {
        let dir = tempfile::tempdir().unwrap();
        Node::new(dir.path().into(), config, None, None).await
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn init_rejects_no_p2p_with_relay() {
        let mut config = NodeConfig::testing();
        config.enable_p2p = false;
        config.use_relay = true;
        assert!(init_result(config).await.is_err());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn init_rejects_no_p2p_with_mdns() {
        use p2panda::network::MdnsDiscoveryMode;
        let mut config = NodeConfig::testing();
        config.enable_p2p = false;
        config.mdns_mode = MdnsDiscoveryMode::Active;
        assert!(init_result(config).await.is_err());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn init_accepts_consistent_no_p2p_config() {
        assert!(init_result(NodeConfig::testing().no_p2p()).await.is_ok());
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
