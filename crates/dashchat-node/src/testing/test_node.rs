use std::{
    collections::{BTreeSet, HashMap, HashSet},
    sync::Arc,
    time::{Duration, Instant},
};

use named_id::*;
use tempfile::TempDir;
use tokio::sync::{Mutex, mpsc::Receiver};

use mailbox_client::{MailboxClient, mem::MemMailbox};

use crate::{
    AgentId, DeviceGroupPayload, NodeConfig, Notification, Payload, Profile,
    mailbox::MailboxOperation, node::Node, testing::behavior::Behavior, topic::TopicId,
};

#[derive(Clone, derive_more::Deref, derive_more::Debug)]
#[debug("TestNode({})", self.node.device_id().renamed())]
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
        let (notification_tx, notification_rx) = tokio::sync::mpsc::channel(100);
        let node = Node::new(dir.path().into(), config.node_config, Some(notification_tx))
            .await
            .unwrap();
        if config.use_named_id {
            node.device_id().with_name(name);
            node.agent_id().with_name(name);
        }
        if config.create_profile {
            node.set_profile(Profile {
                name: name.to_string(),
                avatar: None,
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

    pub async fn add_mailbox_client(&self, mailbox: impl MailboxClient<MailboxOperation>) -> Self {
        self.node.mailboxes.add(mailbox).await;
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

    pub async fn subscribed_topics(&self) -> BTreeSet<TopicId> {
        let mailbox_topics = self.mailboxes.subscribed_topics().await;
        mailbox_topics

        // self.node
        //     .initialized_topics
        //     .read()
        //     .await
        //     .keys()
        //     .cloned()
        //     .chain(mailbox_topics)
        //     .collect::<BTreeSet<_>>()
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
            node_config: NodeConfig::default(),
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

#[derive(Clone, Debug)]
pub struct ClusterConfig {
    pub poll_interval: Duration,
    pub poll_timeout: Duration,
}

impl Default for ClusterConfig {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_millis(100),
            poll_timeout: Duration::from_secs(10),
        }
    }
}

#[derive(derive_more::Deref)]
pub struct TestCluster<const N: usize> {
    #[deref]
    nodes: [TestNode; N],
    pub config: ClusterConfig,
}

impl<const N: usize> TestCluster<N> {
    // TODO: maybe don't always add a memory mailbox
    pub async fn new(node_config: NodeConfig, config: ClusterConfig, aliases: [&str; N]) -> Self {
        let mailbox = MemMailbox::<MailboxOperation>::new();
        let nodes: [TestNode; N] = futures::future::join_all(
            (0..N).map(|i| TestNode::new(node_config.clone(), aliases[i])),
        )
        .await
        .try_into()
        .unwrap_or_else(|_| panic!("expected {} nodes", N));

        for n in nodes.iter() {
            n.add_mailbox_client(mailbox.client()).await;
        }

        Self { nodes, config }
    }

    pub async fn introduce_all(&self) {
        #[cfg(feature = "p2p")]
        {
            let nodes = self.iter().map(|node| &node.network).collect::<Vec<_>>();
            introduce(nodes).await;
        }
    }

    pub async fn nodes(&self) -> [TestNode; N] {
        self.nodes
            .iter()
            .map(|node| node.clone())
            .collect::<Vec<_>>()
            .try_into()
            .unwrap()
    }

    pub async fn consistency(
        &self,
        topics: impl IntoIterator<Item = &TopicId>,
    ) -> anyhow::Result<()> {
        consistency(self.nodes().await.iter(), topics, &self.config).await
    }
}

pub async fn consistency(
    nodes: impl IntoIterator<Item = &TestNode>,
    topics: impl IntoIterator<Item = &TopicId>,
    config: &ClusterConfig,
) -> anyhow::Result<()> {
    let topics = topics.into_iter().collect::<HashSet<_>>();
    let nodes = nodes.into_iter().collect::<Vec<_>>();
    wait_for_resetting(config.poll_interval, config.poll_timeout, || async {
        // TODO: Fix this when we have a proper way to access operations
        // The operations field is now private in the new p2panda-store version
        let sets = nodes
            .iter()
            .map(|node| {
                let ops = node.op_store.processed_ops.read().unwrap();

                topics
                    .iter()
                    .flat_map(|topic| {
                        ops.get(topic)
                            .cloned()
                            .unwrap_or_default()
                            .into_iter()
                            .map(|h| format!("{} {}", h.short(), h.renamed()))
                    })
                    .collect::<BTreeSet<_>>()
            })
            .collect::<Vec<_>>();
        let mut diffs = ConsistencyReport::new(sets);
        for i in 0..diffs.sets.len() {
            for j in 0..i {
                if i != j && diffs.sets[i] != diffs.sets[j] {
                    diffs.diffs.insert(
                        (i, j),
                        (diffs.sets[i].len() as isize - diffs.sets[j].len() as isize).abs(),
                    );
                }
            }
        }
        if diffs.diffs.is_empty() {
            Ok(())
        } else {
            Err(diffs)
        }
    })
    .await
    .map_err(|diffs| {
        for n in nodes {
            println!(
                ">>> {:?}\n{}\n",
                n.device_id(),
                n.op_store.report(topics.clone())
            );
        }
        println!("consistency report: {:#?}", diffs);
        anyhow::anyhow!("consistency check failed")
    })
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ConsistencyReport {
    sets: Vec<BTreeSet<String>>,
    diffs: HashMap<(usize, usize), isize>,
}

impl ConsistencyReport {
    pub fn new(sets: Vec<BTreeSet<String>>) -> Self {
        Self {
            sets,
            diffs: HashMap::new(),
        }
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
        let timeout = tokio::time::sleep(timeout);
        tokio::pin!(timeout);

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
                _ = &mut timeout => return Err(anyhow::anyhow!("timeout")),
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

pub async fn wait_for<F, E>(poll: Duration, timeout: Duration, f: impl Fn() -> F) -> Result<(), E>
where
    F: Future<Output = Result<(), E>>,
{
    assert!(poll < timeout);
    let start = Instant::now();
    tracing::info!("=== wait_for() up to {:?} ===", timeout);
    loop {
        let result = f().await;
        match &result {
            Ok(()) => break,
            Err(_) => {
                if start.elapsed() > timeout {
                    return result;
                }

                tokio::time::sleep(poll).await;
            }
        }
    }
    tracing::info!("=== wait_for() success after {:?} ===", start.elapsed());
    Ok(())
}

pub async fn wait_for_resetting<F, E>(
    poll: Duration,
    timeout: Duration,
    f: impl Fn() -> F,
) -> Result<(), E>
where
    F: Future<Output = Result<(), E>>,
    E: std::fmt::Debug + PartialEq,
{
    assert!(poll < timeout);
    let mut start = Instant::now();
    tracing::info!("=== wait_for_resetting() up to {:?} ===", timeout);
    let mut previous = None;
    loop {
        let result = f().await;
        match &result {
            Ok(()) => break,
            Err(_) => {
                if start.elapsed() > timeout {
                    return result;
                }

                if previous.as_ref() != Some(&result) {
                    start = Instant::now();
                }

                previous = Some(result);

                tokio::time::sleep(poll).await;
            }
        }
    }
    tracing::info!(
        "=== wait_for_resetting() success after {:?} ===",
        start.elapsed()
    );
    Ok(())
}
