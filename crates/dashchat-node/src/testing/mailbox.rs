use std::path::PathBuf;

use mailbox_client::{
    FetchRequest, FetchResponse, MailboxClient, MailboxId, RegisterPeerRequest,
    mem::{MemMailbox, MemMailboxClient},
    toy::ToyMailboxClient,
};

use crate::mailbox::MailboxOperation;

/// Env var naming the deployment environment whose cloud mailbox the test
/// suite should run against (e.g. `DASHCHAT_TEST_ENV=testing`). When unset,
/// tests use an in-memory mailbox.
pub const TEST_ENV_VAR: &str = "DASHCHAT_TEST_ENV";

/// Deployment environments the test suite is allowed to run against.
pub const ALLOWED_TEST_ENVS: &[&str] = &["testing"];

/// A mailbox for tests, built by [`TestMailbox::from_env`]: an in-memory
/// mailbox by default, or the cloud mailbox of a deployment environment when
/// [`TEST_ENV_VAR`] is set.
#[derive(Clone)]
pub enum TestMailbox {
    Mem(MemMailbox<MailboxOperation>),
    Env { name: String, url: String },
}

impl TestMailbox {
    /// Builds a mailbox according to [`TEST_ENV_VAR`]: unset or empty → a
    /// fresh [`MemMailbox`]; an allowlisted environment name → that
    /// environment's cloud mailbox (URL resolved from the repo's
    /// `.env.<name>` file). Panics on a non-allowlisted environment.
    pub fn from_env() -> Self {
        match std::env::var(TEST_ENV_VAR) {
            Err(_) => Self::Mem(MemMailbox::new()),
            Ok(name) if name.is_empty() => Self::Mem(MemMailbox::new()),
            Ok(name) => {
                assert!(
                    ALLOWED_TEST_ENVS.contains(&name.as_str()),
                    "{TEST_ENV_VAR}={name} is not an allowed test environment (allowed: {ALLOWED_TEST_ENVS:?})"
                );
                let url = env_file_mailbox_url(&name);
                Self::Env { name, url }
            }
        }
    }

    /// The mailbox's id: the in-memory id, or the environment mailbox's
    /// canonical id resolved from its `/health` endpoint.
    pub async fn id(&self) -> MailboxId {
        match self {
            Self::Mem(mb) => mb.client().id(),
            Self::Env { url, .. } => fetch_mailbox_health(url).await.unwrap().mailbox_id,
        }
    }

    /// A standalone client with a throwaway sender identity. Prefer
    /// [`crate::testing::TestNode::add_mailbox`], which attributes the client
    /// to the node and registers addresses for blob transfer.
    pub async fn client(&self) -> TestMailboxClient {
        match self {
            Self::Mem(mb) => TestMailboxClient::Mem(mb.client()),
            Self::Env { url, .. } => {
                let health = fetch_mailbox_health(url).await.unwrap();
                TestMailboxClient::Toy(ToyMailboxClient::new(
                    health.mailbox_id,
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
            Self::Env { url, .. } => {
                let health = fetch_mailbox_health(url).await.unwrap();
                node.insert_peer_addr(health.endpoint_addr).await.unwrap();
                node.mailboxes
                    .register(ToyMailboxClient::<MailboxOperation>::new(
                        health.mailbox_id,
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

/// Reads `MAILBOX_URL` from the repo-root `.env.<name>` file, resolving
/// `${VAR}` references against values defined earlier in the same file.
fn env_file_mailbox_url(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("../../.env.{name}"));
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
    let mut vars: Vec<(String, String)> = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let mut value = value.trim().to_string();
        for (k, v) in &vars {
            value = value.replace(&format!("${{{k}}}"), v);
        }
        vars.push((key.trim().to_string(), value));
    }
    vars.into_iter()
        .find(|(k, _)| k == "MAILBOX_URL")
        .map(|(_, v)| v)
        .unwrap_or_else(|| panic!("no MAILBOX_URL in {}", path.display()))
}

struct MailboxHealth {
    mailbox_id: MailboxId,
    endpoint_addr: iroh::EndpointAddr,
}

/// Fetch a mailbox server's `/health` response: its canonical MailboxId and
/// its dialing address (mirrors `src-tauri/src/setup.rs`).
async fn fetch_mailbox_health(base_url: &str) -> anyhow::Result<MailboxHealth> {
    #[derive(serde::Deserialize)]
    struct HealthResponse {
        endpoint_id: String,
        endpoint_addr: iroh::EndpointAddr,
    }
    let url = format!("{}/health", base_url.trim_end_matches('/'));
    let resp = mailbox_client::HTTP_CLIENT
        .get(&url)
        .send()
        .await?
        .error_for_status()?
        .json::<HealthResponse>()
        .await?;
    Ok(MailboxHealth {
        mailbox_id: resp.endpoint_id,
        endpoint_addr: resp.endpoint_addr,
    })
}

/// POST the node's `EndpointAddr` to the mailbox's `/peers/register` endpoint
/// so its blob fetch pool can dial the node as a source.
async fn register_self_with_mailbox(
    base_url: &str,
    our_addr: iroh::EndpointAddr,
) -> anyhow::Result<()> {
    let url = format!("{}/peers/register", base_url.trim_end_matches('/'));
    mailbox_client::HTTP_CLIENT
        .post(&url)
        .json(&RegisterPeerRequest { addr: our_addr })
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_file_mailbox_url_interpolates_domain() {
        let url = env_file_mailbox_url("testing");
        assert_eq!(url, "https://0-19.mailbox.testing.darksoil.studio");
    }
}
