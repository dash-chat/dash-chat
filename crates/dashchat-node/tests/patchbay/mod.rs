//! Nodes and mailboxes on a virtual network whose links between regions can
//! be slowed down, built on patchbay's Linux network namespaces.

use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use dashchat_node::NodeConfig;
use dashchat_node::mailbox::MailboxOperation;
use dashchat_node::testing::TestNode;
use mailbox_client::MailboxClient;
use mailbox_client::backends::toy::ToyMailboxClient;
use patchbay::{Device, Region, RegionLink};
use tokio::task::JoinHandle;

use crate::common;

/// Result of entering a user namespace, captured before the test harness spawns
/// additional threads. On kernels or containers that disallow unprivileged user
/// namespaces the initialization fails; tests that need it can then skip rather
/// than abort.
static USERNS: OnceLock<Result<(), String>> = OnceLock::new();

/// A process can only enter a user namespace while it has a single thread,
/// and the test harness starts its threads before any test runs.
#[ctor::ctor(unsafe)]
fn enter_user_namespace() {
    let result = patchbay::init_userns().map_err(|e| format!("{e}"));
    // Ignore set errors: this runs once per process under ctor.
    let _ = USERNS.set(result);
}

/// Whether the user namespace was entered successfully. Tests that require
/// patchbay's network isolation should check this and skip gracefully.
pub fn userns_available() -> Result<(), String> {
    USERNS
        .get()
        .cloned()
        .unwrap_or_else(|| Err("user namespace initialization did not run".to_string()))
}

pub enum Rate {
    Mbit(u32),
    Unlimited,
}

pub struct Network {
    lab: patchbay::Lab,
    /// Each node reaches the mailbox through a link of its own: the nodes of a
    /// test share one HTTP client, which would otherwise hand a node a
    /// connection another node opened in its own namespace.
    mailbox_links: Mutex<Vec<common::MailboxLink>>,
}

impl Network {
    pub async fn new() -> anyhow::Result<Self> {
        userns_available().map_err(|e| anyhow::anyhow!(e))?;
        Ok(Self {
            lab: patchbay::Lab::new().await?,
            mailbox_links: Mutex::default(),
        })
    }

    /// patchbay names the link between two regions after both, and Linux caps
    /// interface names at 15 characters, so region names must stay short.
    pub async fn regions<const N: usize>(&self, names: [&str; N]) -> anyhow::Result<[Region; N]> {
        let mut regions = Vec::with_capacity(N);
        for name in names {
            regions.push(self.lab.add_region(name).await?);
        }
        Ok(regions.try_into().ok().expect("one region per name"))
    }

    pub async fn link(&self, a: &Region, b: &Region, rate: Rate) -> anyhow::Result<()> {
        let link = match rate {
            Rate::Mbit(rate_mbit) => RegionLink {
                rate_mbit,
                ..RegionLink::good(1)
            },
            Rate::Unlimited => RegionLink::good(1),
        };
        self.lab.link_regions(a, b, link).await
    }

    pub async fn mailbox_in(&self, region: &Region) -> anyhow::Result<Mailbox> {
        let device = self.device_in(region, "mailbox").await?;
        let ip = device.ip().expect("a lab device has an address");
        let server = device
            .spawn(async move |_| common::spawn_standalone_mailbox_on(ip.into()).await)?
            .await?;
        Ok(Mailbox { server, device })
    }

    /// A node on a device of its own in `region`, registered with `mailbox`
    /// the way the app registers the cloud mailbox.
    pub async fn node_in(
        &self,
        region: &Region,
        name: &'static str,
        mailbox: &Mailbox,
    ) -> anyhow::Result<TestNode> {
        let device = self.device_in(region, name).await?;
        let (id, url, addr) = (
            mailbox.server.id.clone(),
            mailbox.server.url.clone(),
            mailbox.server.endpoint_addr.clone(),
        );
        let (node, link) = device
            .spawn(async move |_| {
                let node = TestNode::new(lab_node_config(), name).await;
                let link = common::MailboxLink::spawn(&url).await;
                node.insert_peer_addr(addr).await.unwrap();
                let client = ToyMailboxClient::<MailboxOperation>::new(
                    id,
                    &link.url,
                    node.endpoint_id(),
                    node.unfetched_blob_tracker(),
                    node.mailboxes.upload_tracker(),
                )
                .with_blob_reader(node.blob_reader())
                .with_blob_pusher(node.blob_pusher().await);
                node.add_mailbox_client(client).await;
                (node, link)
            })?
            .await?;
        self.mailbox_links.lock().unwrap().push(link);
        Ok(node)
    }

    async fn device_in(&self, region: &Region, name: &str) -> anyhow::Result<Device> {
        let router = self
            .lab
            .add_router(&format!("{name}-net"))
            .region(region)
            .build()
            .await?;
        self.lab.add_device(name).uplink(router.id()).build().await
    }
}

fn lab_node_config() -> NodeConfig {
    let mut config = NodeConfig::testing();
    // The test settings give a fetch 3 s, which carries next to nothing over a
    // slow link, so lab nodes fetch as the app does.
    config.blob_fetch = dashchat_utils::FetchConfig::default();
    config
}

pub struct Mailbox {
    server: common::StandaloneMailbox,
    device: Device,
}

impl Mailbox {
    /// Resolves to the moment the mailbox first holds `hash`.
    pub fn when_it_holds(&self, hash: iroh_blobs::Hash) -> anyhow::Result<JoinHandle<Instant>> {
        let inspector = common::inspection_client(&self.server.id, &self.server.url);
        Ok(self.device.spawn(async move |_| {
            loop {
                let stored = inspector.announce_blob_hashes(vec![hash], false).await;
                if stored.is_ok_and(|stored| stored.contains(&hash)) {
                    return Instant::now();
                }
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        })?)
    }
}
