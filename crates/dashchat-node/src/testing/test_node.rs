use std::{
    collections::{BTreeSet, HashSet},
    sync::Arc,
    time::{Duration, Instant},
};

use aliased::Aliasing;
use p2panda::operation::{Header, LogId, Operation};
use p2panda_store::operations::OperationStore;
use tempfile::TempDir;
use tokio::sync::{Mutex, mpsc::Receiver};

use mailbox_client::MailboxClient;

use crate::{
    AgentId, DeviceGroupPayload, NodeConfig, Notification, Payload, Profile,
    filesystem::Filesystem, mailbox::MailboxOperation, node::Node, stores::LocalStore,
    testing::behavior::Behavior, topic::TopicId,
};

#[derive(Clone, derive_more::Deref, derive_more::Debug)]
#[debug("TestNode({:?})", self.node.device_id().aliased())]
pub struct TestNode {
    #[deref]
    node: Node,
    pub watcher: Arc<Mutex<Watcher<Notification>>>,

    // store temp directory is deleted when this is dropped
    _store_dir: Arc<TempDir>,
}

impl TestNode {
    pub async fn new(config: impl Into<TestNodeConfig>, name: &str) -> Self {
        let config = config.into();
        let dir = tempfile::tempdir().unwrap();
        tracing::info!("temp storage dir: {}", dir.path().display());
        let (notification_tx, notification_rx) = tokio::sync::mpsc::channel(100);

        let filesystem = Filesystem::new(dir.path().to_path_buf());
        let local_store = LocalStore::new(filesystem.local_store_path())
            .await
            .unwrap();
        if config.use_named_id {
            local_store.device_id().await.unwrap().alias_named(name);
            local_store.agent_id().await.unwrap().alias_named(name);
        }
        drop(local_store);

        let node = Node::new(
            dir.path().into(),
            config.node_config,
            Some(notification_tx),
            None,
        )
        .await
        .unwrap();
        if config.create_profile {
            node.set_profile(Profile {
                name: name.to_string(),
                surname: None,
                avatar: None,
                about: None,
            })
            .await
            .unwrap();
        }
        Self {
            node,
            watcher: Arc::new(Mutex::new(Watcher(notification_rx))),
            _store_dir: Arc::new(dir),
        }
    }

    /// Returns the store directory so it can be kept alive after dropping the TestNode.
    /// Useful for restart scenarios where you need to create a new node at the same path.
    pub fn store_dir(&self) -> Arc<TempDir> {
        self._store_dir.clone()
    }

    /// Creates a TestNode at an existing filesystem path (for restart scenarios).
    /// Skips profile creation since it already exists in the persisted op store.
    /// Re-registers named IDs for debug output since the in-memory registry is lost on drop.
    pub async fn new_at_path(config: NodeConfig, name: &str, store_dir: Arc<TempDir>) -> Self {
        let (notification_tx, notification_rx) = tokio::sync::mpsc::channel(100);

        let filesystem = Filesystem::new(store_dir.path().to_path_buf());
        let local_store = LocalStore::new(filesystem.local_store_path())
            .await
            .unwrap();
        local_store.device_id().await.unwrap().alias_named(name);
        local_store.agent_id().await.unwrap().alias_named(name);
        drop(local_store);

        let node = Node::new(store_dir.path().into(), config, Some(notification_tx), None)
            .await
            .unwrap();

        Self {
            node,
            watcher: Arc::new(Mutex::new(Watcher(notification_rx))),
            _store_dir: store_dir,
        }
    }

    /// Shut down the node's background tasks and return the store directory.
    /// The store directory can be passed to `new_at_path` to restart the node.
    pub async fn shutdown(self) -> Arc<TempDir> {
        let dir = self._store_dir.clone();
        self.node.shutdown().await.unwrap();
        dir
    }

    pub async fn add_mailbox_client(&self, mailbox: impl MailboxClient<MailboxOperation>) -> Self {
        self.node.mailboxes.register(mailbox).await;
        self.clone()
    }

    pub async fn clear_mailboxes(&self) {
        self.node.mailboxes.clear().await;
    }

    pub fn behavior(&self) -> Behavior {
        Behavior::new(self.clone())
    }

    pub async fn get_contacts(&self) -> anyhow::Result<Vec<AgentId>> {
        // FIXME: use all local device IDs
        let ids = self
            .op_store
            .get_interleaved_logs(self.device_group_topic().into(), vec![self.device_id()])
            .await?
            .into_iter()
            .filter_map(|(_, payload)| match payload {
                Some(Payload::DeviceGroup(DeviceGroupPayload::AddContact(qr))) => Some(qr.agent_id),
                _ => None,
            })
            .collect();
        Ok(ids)
    }

    pub async fn get_rejected_contact_requests(&self) -> anyhow::Result<Vec<AgentId>> {
        let ids = self
            .op_store
            .get_interleaved_logs(self.device_group_topic().into(), vec![self.device_id()])
            .await?
            .into_iter()
            .filter_map(|(_, payload)| match payload {
                Some(Payload::DeviceGroup(DeviceGroupPayload::RejectContactRequest(agent_id))) => {
                    Some(agent_id)
                }
                _ => None,
            })
            .collect();
        Ok(ids)
    }

    pub async fn subscribed_topics(&self) -> BTreeSet<LogId> {
        self.mailboxes.subscribed_topics().await
    }
}

#[derive(Clone, Debug)]
pub struct TestNodeConfig {
    /// The config to pass on to the node
    pub node_config: NodeConfig,
    /// Create an initial profile before returning
    pub create_profile: bool,
    /// Use a named-id for the device and agent IDs
    pub use_named_id: bool,
}

impl Default for TestNodeConfig {
    fn default() -> Self {
        Self {
            node_config: NodeConfig::testing(),
            create_profile: true,
            use_named_id: true,
        }
    }
}

impl From<NodeConfig> for TestNodeConfig {
    fn from(node_config: NodeConfig) -> Self {
        Self {
            node_config,
            ..Default::default()
        }
    }
}

/// Config for operations that involve polling and waiting for conditions to be met.
#[derive(Clone, Debug)]
pub struct PollConfig {
    pub poll_interval: Duration,
    pub poll_timeout: Duration,
}

impl Default for PollConfig {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_millis(100),
            poll_timeout: Duration::from_secs(10),
        }
    }
}

impl PollConfig {
    pub async fn consistency(
        &self,
        nodes: impl IntoIterator<Item = &TestNode>,
        topics: impl IntoIterator<Item = &TopicId>,
    ) -> anyhow::Result<()> {
        let topics = topics.into_iter().collect::<HashSet<_>>();
        let nodes = nodes.into_iter().collect::<Vec<_>>();
        self.wait_for_resetting(|| async {
            // TODO: Fix this when we have a proper way to access operations
            // The operations field is now private in the new p2panda-store version
            let sets = nodes
                .iter()
                .map(|&node| {
                    let ops = node.op_store.processed_ops.read().unwrap();
                    let hashes = topics
                        .iter()
                        .flat_map(|topic| ops.get(topic).cloned().unwrap_or_default().into_iter())
                        .collect::<BTreeSet<_>>();
                    (node.clone(), hashes)
                })
                .collect::<Vec<_>>();
            let report = ConsistencyReport::new(sets).await.unwrap();
            if report.passes() { Ok(()) } else { Err(report) }
        })
        .await
        .map_err(|report| {
            println!("--------------------------------");
            println!("{:?}", report);
            println!("--------------------------------");
            anyhow::anyhow!("consistency check failed after {:?}", self.poll_timeout)
        })
    }

    pub async fn wait_for<F, E>(&self, f: impl Fn() -> F) -> Result<(), E>
    where
        F: Future<Output = Result<(), E>>,
    {
        assert!(self.poll_interval < self.poll_timeout);
        let start = Instant::now();
        tracing::info!("=== wait_for() up to {:?} ===", self.poll_timeout);
        loop {
            let result = f().await;
            match &result {
                Ok(()) => break,
                Err(_) => {
                    if start.elapsed() > self.poll_timeout {
                        return result;
                    }

                    tokio::time::sleep(self.poll_interval).await;
                }
            }
        }
        tracing::info!("=== wait_for() success after {:?} ===", start.elapsed());
        Ok(())
    }

    pub async fn wait_for_resetting<F, E>(&self, f: impl Fn() -> F) -> Result<(), E>
    where
        F: Future<Output = Result<(), E>>,
        E: std::fmt::Debug + PartialEq,
    {
        assert!(self.poll_interval < self.poll_timeout);
        let mut start = Instant::now();
        tracing::info!("=== wait_for_resetting() up to {:?} ===", self.poll_timeout);
        let mut previous = None;
        loop {
            let result = f().await;
            match &result {
                Ok(()) => break,
                Err(_) => {
                    if start.elapsed() > self.poll_timeout {
                        return result;
                    }

                    if previous.as_ref() != Some(&result) {
                        start = Instant::now();
                    }

                    previous = Some(result);

                    tokio::time::sleep(self.poll_interval).await;
                }
            }
        }
        tracing::info!(
            "=== wait_for_resetting() success after {:?} ===",
            start.elapsed()
        );
        Ok(())
    }
}

pub async fn consistency(
    nodes: impl IntoIterator<Item = &TestNode>,
    topics: impl IntoIterator<Item = &TopicId>,
) -> anyhow::Result<()> {
    PollConfig::default().consistency(nodes, topics).await
}

#[derive(Clone, Default)]
pub struct ConsistencyReport {
    ops: Vec<(TestNode, Vec<(p2panda::Hash, Header)>)>,
}

impl ConsistencyReport {
    pub async fn new(hashes: Vec<(TestNode, BTreeSet<p2panda::Hash>)>) -> anyhow::Result<Self> {
        let mut nodes = vec![];
        for (node, hashes) in hashes.iter() {
            let mut headers = vec![];
            for hash in hashes {
                let op = OperationStore::<Operation, p2panda::Hash, LogId>::get_operation(
                    &node.op_store.store,
                    hash,
                )
                .await?
                .unwrap();
                headers.push((*hash, op.header));
            }
            headers.sort_by_key(|op| Self::op_line(op.clone()));
            nodes.push((node.clone(), headers));
        }
        Ok(Self { ops: nodes })
    }

    pub fn passes(&self) -> bool {
        let mut digests = HashSet::new();
        for (_, headers) in self.ops.iter() {
            let hashes = headers
                .iter()
                .map(|(hash, _)| hash)
                .collect::<BTreeSet<_>>();
            digests.insert(hashes);
        }
        digests.len() <= 1
    }

    fn op_line((hash, header): (p2panda::Hash, Header)) -> String {
        format!(
            "{:32?} {:3} {:32?}",
            header.extensions.log_id.aliased(),
            header.seq_num,
            hash.aliased()
        )
    }
}

impl std::fmt::Debug for ConsistencyReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (node, headers) in self.ops.iter() {
            writeln!(f, "=== {:?} ===", node.device_id().aliased())?;
            for (hash, header) in headers.iter() {
                writeln!(f, "- {}", Self::op_line((*hash, header.clone())),)?;
            }
        }
        Ok(())
    }
}

impl PartialEq for ConsistencyReport {
    fn eq(&self, other: &Self) -> bool {
        self.ops.iter().zip(other.ops.iter()).all(
            |((left_node, left_ops), (right_node, right_ops))| {
                left_node.device_id() == right_node.device_id() && left_ops == right_ops
            },
        )
    }
}

#[derive(derive_more::Deref, derive_more::DerefMut)]
pub struct Watcher<T>(Receiver<T>);

impl<T: std::fmt::Debug> Watcher<T> {
    pub async fn watch_mapped<R>(
        &mut self,
        timeout: tokio::time::Duration,
        f: impl Fn(&T) -> Option<R>,
    ) -> anyhow::Result<R> {
        let sleep = tokio::time::sleep(timeout);
        tokio::pin!(sleep);

        loop {
            tokio::select! {
                item = self.0.recv() => {
                    match item {
                        Some(item) => match f(&item) {
                            Some(r) => return Ok(r),
                            None => continue,
                        },
                        None => return Err(anyhow::anyhow!("channel closed")),
                    }
                }
                _ = &mut sleep => return Err(anyhow::anyhow!("timeout after {:?}", timeout)),
            }
        }
    }

    pub async fn watch_for(
        &mut self,
        timeout: tokio::time::Duration,
        f: impl Fn(&T) -> bool,
    ) -> anyhow::Result<T> {
        let timeout = tokio::time::sleep(timeout);
        tokio::pin!(timeout);

        loop {
            tokio::select! {
                item = self.0.recv() => {
                    match item {
                        Some(item) => if f(&item) {
                            return Ok(item)
                        } else {
                            continue
                        },
                        None => return Err(anyhow::anyhow!("channel closed")),
                    }
                }
                _ = &mut timeout => return Err(anyhow::anyhow!("timeout")),
            }
        }
    }
}
