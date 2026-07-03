use mailbox_client::{
    FetchRequest, FetchResponse, MailboxClient, MailboxId,
    mem::{MemMailbox, MemMailboxClient},
    toy::ToyMailboxClient,
};
use std::sync::LazyLock;

use regex::Regex;

use crate::mailbox::{
    MailboxHealth, MailboxOperation, fetch_mailbox_health, register_self_with_mailbox,
};

/// Regex patterns for the mailbox URLs the test suite is allowed to run
/// against, shared with the E2E suite via `allowed-test-mailbox-url-patterns.json`
/// at the repo root. Any `MAILBOX_URL` matching none of them fails fast so
/// tests can never hit staging or production.
const ALLOWED_MAILBOX_URL_PATTERNS_JSON: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../allowed-test-mailbox-url-patterns.json"
));

static ALLOWED_MAILBOX_URL_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    serde_json::from_str::<Vec<String>>(ALLOWED_MAILBOX_URL_PATTERNS_JSON)
        .expect("allowed-test-mailbox-url-patterns.json is a JSON array of strings")
        .iter()
        .map(|pattern| {
            Regex::new(pattern).expect("invalid regex in allowed-test-mailbox-url-patterns.json")
        })
        .collect()
});

/// A mailbox for tests, built by [`TestMailbox::from_env`]: an in-memory
/// mailbox by default, or a cloud mailbox when `MAILBOX_URL` names an
/// allowlisted deployment environment.
#[derive(Clone)]
pub enum TestMailbox {
    Mem(MemMailbox<MailboxOperation>),
    Env { url: String },
}

impl TestMailbox {
    /// Builds a mailbox from `MAILBOX_URL`: unset or empty → a fresh
    /// [`MemMailbox`]; an allowlisted URL → that environment's cloud mailbox.
    /// Panics on a non-allowlisted URL.
    pub fn from_env() -> Self {
        match std::env::var("MAILBOX_URL")
            .ok()
            .filter(|url| !url.is_empty())
        {
            None => Self::Mem(MemMailbox::new()),
            Some(url) => {
                assert!(
                    ALLOWED_MAILBOX_URL_PATTERNS
                        .iter()
                        .any(|pattern| pattern.is_match(&url)),
                    "MAILBOX_URL={url} is not an allowed test mailbox (allowed patterns: {ALLOWED_MAILBOX_URL_PATTERNS_JSON})"
                );
                Self::Env { url }
            }
        }
    }

    async fn health(&self) -> MailboxHealth {
        let Self::Env { url } = self else {
            unreachable!("health() only called on the Env variant")
        };
        fetch_mailbox_health(url).await.unwrap()
    }

    /// The mailbox's id: the in-memory id, or the environment mailbox's
    /// canonical id resolved from its `/health` endpoint.
    pub async fn id(&self) -> MailboxId {
        match self {
            Self::Mem(mb) => mb.client().id(),
            Self::Env { .. } => self.health().await.mailbox_id.clone(),
        }
    }

    /// A standalone client with a throwaway sender identity. Prefer
    /// [`crate::testing::TestNode::add_mailbox`], which attributes the client
    /// to the node and registers addresses for blob transfer.
    pub async fn client(&self) -> TestMailboxClient {
        match self {
            Self::Mem(mb) => TestMailboxClient::Mem(mb.client()),
            Self::Env { url } => {
                let health = self.health().await;
                TestMailboxClient::Toy(ToyMailboxClient::new(
                    health.mailbox_id.clone(),
                    url,
                    iroh::SecretKey::generate().public(),
                ))
            }
        }
    }

    /// Registers this mailbox on a node the way the production app does: for
    /// an environment mailbox, resolve its id from `/health`, add its dialing
    /// address to the node's address book, and register the node's own
    /// address back so the mailbox's blob fetcher can dial it.
    pub async fn register_on(&self, node: &crate::Node) {
        match self {
            Self::Mem(mb) => node.mailboxes.register(mb.client()).await,
            Self::Env { url } => {
                let health = self.health().await;
                node.insert_peer_addr(health.endpoint_addr.clone())
                    .await
                    .unwrap();
                node.mailboxes
                    .register(ToyMailboxClient::<MailboxOperation>::new(
                        health.mailbox_id.clone(),
                        url,
                        node.endpoint_id(),
                    ))
                    .await;
                register_self_with_mailbox(url, node.iroh_endpoint().await.unwrap().addr())
                    .await
                    .unwrap();
            }
        }
    }
}

/// Client produced by [`TestMailbox::client`], delegating to the in-memory or
/// HTTP client underneath.
#[derive(Clone)]
pub enum TestMailboxClient {
    Mem(MemMailboxClient<MailboxOperation>),
    Toy(ToyMailboxClient<MailboxOperation>),
}

#[async_trait::async_trait]
impl MailboxClient<MailboxOperation> for TestMailboxClient {
    fn id(&self) -> MailboxId {
        match self {
            Self::Mem(client) => client.id(),
            Self::Toy(client) => client.id(),
        }
    }

    fn url(&self) -> Option<String> {
        match self {
            Self::Mem(client) => client.url(),
            Self::Toy(client) => client.url(),
        }
    }

    async fn publish(&self, ops: Vec<MailboxOperation>) -> Result<(), anyhow::Error> {
        match self {
            Self::Mem(client) => client.publish(ops).await,
            Self::Toy(client) => client.publish(ops).await,
        }
    }

    async fn fetch(
        &self,
        request: FetchRequest<MailboxOperation>,
    ) -> Result<FetchResponse<MailboxOperation>, anyhow::Error> {
        match self {
            Self::Mem(client) => client.fetch(request).await,
            Self::Toy(client) => client.fetch(request).await,
        }
    }
}
