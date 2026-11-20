//! A drop-in for `reqwest::Client` whose requests do not outlive the network
//! they started on.
//!
//! When the network goes away the OS can leave open sockets in place (iOS
//! does), and a request on one hangs until its timeout instead of failing at
//! once. reqwest offers no way to drop its pooled connections, so a network
//! change fails the requests in flight, and later requests go through a client
//! built afresh.

use std::sync::Mutex;
use std::time::Duration;

use tokio::sync::broadcast;

/// A request that did not get a response.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Http(#[from] reqwest::Error),
    #[error("the network changed while the request was in flight")]
    NetworkChanged,
}

impl Error {
    /// Whether the server could not be reached at all.
    pub fn is_connect(&self) -> bool {
        match self {
            Error::Http(err) => err.is_connect(),
            Error::NetworkChanged => true,
        }
    }
}

/// A `reqwest::Client` built from `configure`, and built again from it
/// whenever the network changes.
pub struct Client {
    configure: fn() -> reqwest::ClientBuilder,
    current: Mutex<Current>,
}

struct Current {
    client: reqwest::Client,
    /// Subscribed on first use, since subscribing starts the watcher on the
    /// tokio runtime.
    network_changes: Option<broadcast::Receiver<()>>,
}

impl Client {
    pub fn new(configure: fn() -> reqwest::ClientBuilder) -> Result<Self, reqwest::Error> {
        Ok(Self {
            configure,
            current: Mutex::new(Current {
                client: configure().build()?,
                network_changes: None,
            }),
        })
    }

    pub fn get(&self, url: impl reqwest::IntoUrl) -> RequestBuilder {
        self.request(reqwest::Method::GET, url)
    }

    pub fn post(&self, url: impl reqwest::IntoUrl) -> RequestBuilder {
        self.request(reqwest::Method::POST, url)
    }

    pub fn request(&self, method: reqwest::Method, url: impl reqwest::IntoUrl) -> RequestBuilder {
        // Subscribed before the client is picked, so a change landing after
        // that still fails the request.
        let network_changes = network_watch::network_change();
        RequestBuilder {
            inner: self.current().request(method, url),
            network_changes,
        }
    }

    /// The reqwest client, built afresh first if the network changed since it
    /// was last used.
    fn current(&self) -> reqwest::Client {
        let mut current = self.current.lock().unwrap();
        let changes = current
            .network_changes
            .get_or_insert_with(network_watch::network_change);
        let mut changed = false;
        loop {
            match changes.try_recv() {
                Ok(()) | Err(broadcast::error::TryRecvError::Lagged(_)) => changed = true,
                Err(
                    broadcast::error::TryRecvError::Empty | broadcast::error::TryRecvError::Closed,
                ) => {
                    break;
                }
            }
        }
        if changed {
            match (self.configure)().build() {
                Ok(client) => current.client = client,
                Err(err) => {
                    tracing::warn!(
                        ?err,
                        "could not rebuild the HTTP client after a network change"
                    )
                }
            }
        }
        current.client.clone()
    }
}

impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client").finish_non_exhaustive()
    }
}

/// A `reqwest::RequestBuilder` whose `send` fails if the network changes
/// before the response arrives.
pub struct RequestBuilder {
    inner: reqwest::RequestBuilder,
    network_changes: broadcast::Receiver<()>,
}

impl RequestBuilder {
    pub fn json<T: serde::Serialize + ?Sized>(self, json: &T) -> Self {
        Self {
            inner: self.inner.json(json),
            ..self
        }
    }

    pub fn body(self, body: impl Into<reqwest::Body>) -> Self {
        Self {
            inner: self.inner.body(body),
            ..self
        }
    }

    pub fn timeout(self, timeout: Duration) -> Self {
        Self {
            inner: self.inner.timeout(timeout),
            ..self
        }
    }

    pub async fn send(self) -> Result<reqwest::Response, Error> {
        send_until_change(self.inner, self.network_changes).await
    }
}

async fn send_until_change(
    request: reqwest::RequestBuilder,
    mut network_changes: broadcast::Receiver<()>,
) -> Result<reqwest::Response, Error> {
    tokio::select! {
        response = request.send() => Ok(response?),
        _ = next_change(&mut network_changes) => Err(Error::NetworkChanged),
    }
}

/// Resolves on the next network change, and never once the signal is gone.
async fn next_change(network_changes: &mut broadcast::Receiver<()>) {
    if let Err(broadcast::error::RecvError::Closed) = network_changes.recv().await {
        std::future::pending::<()>().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The address of a server that accepts connections and never answers,
    /// like a socket whose network went away.
    async fn silent_server() -> std::net::SocketAddr {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let mut held = Vec::new();
            loop {
                held.push(listener.accept().await.unwrap());
            }
        });
        addr
    }

    #[tokio::test]
    async fn a_network_change_fails_a_request_waiting_on_a_silent_server() {
        let addr = silent_server().await;
        let (changes, network_changes) = broadcast::channel(1);
        let request = reqwest::Client::new().get(format!("http://{addr}/"));
        let pending = tokio::spawn(send_until_change(request, network_changes));
        tokio::time::sleep(Duration::from_millis(200)).await;

        changes.send(()).unwrap();

        let result = tokio::time::timeout(Duration::from_secs(1), pending)
            .await
            .expect("the request kept waiting after the network changed")
            .unwrap();
        assert!(matches!(result, Err(Error::NetworkChanged)));
    }

    #[tokio::test]
    async fn a_network_change_between_building_and_sending_fails_the_request() {
        let addr = silent_server().await;
        let (changes, network_changes) = broadcast::channel(1);
        let request = RequestBuilder {
            inner: reqwest::Client::new().get(format!("http://{addr}/")),
            network_changes,
        };

        changes.send(()).unwrap();

        let result = tokio::time::timeout(Duration::from_secs(1), request.send())
            .await
            .expect("the request missed the change that landed before it was sent");
        assert!(matches!(result, Err(Error::NetworkChanged)));
    }

    #[tokio::test]
    async fn a_closed_network_signal_leaves_requests_to_their_own_timeout() {
        let addr = silent_server().await;
        let (changes, network_changes) = broadcast::channel(1);
        drop(changes);
        let request = reqwest::Client::new()
            .get(format!("http://{addr}/"))
            .timeout(Duration::from_millis(300));

        let result = send_until_change(request, network_changes).await;

        assert!(matches!(result, Err(Error::Http(err)) if err.is_timeout()));
    }
}
