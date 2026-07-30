use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use sentry::protocol::{Breadcrumb, Event};
use sentry::{BeforeCallback, ClientOptions};

use crate::Config;

const MAX_BREADCRUMBS: usize = 300;
const MAX_PENDING: usize = 20;

/// Everything collected so far that has not been sent: the recent breadcrumb
/// trail, and events awaiting the user's Send.
///
/// Breadcrumbs live here rather than on sentry's scope because a `Hub` is
/// thread-local — crumbs logged on a tokio worker would be invisible to an event
/// captured on a Tauri command thread.
pub(crate) struct Captured {
    breadcrumbs: Mutex<VecDeque<Breadcrumb>>,
    pending: Mutex<VecDeque<Event<'static>>>,
}

impl Captured {
    pub(crate) fn new() -> Self {
        Self {
            breadcrumbs: Mutex::new(VecDeque::new()),
            pending: Mutex::new(VecDeque::new()),
        }
    }

    pub(crate) fn add_breadcrumb(&self, crumb: Breadcrumb) {
        let mut buf = self.breadcrumbs.lock().unwrap_or_else(|e| e.into_inner());
        buf.push_back(crumb);
        while buf.len() > MAX_BREADCRUMBS {
            buf.pop_front();
        }
    }

    fn snapshot_breadcrumbs(&self) -> Vec<Breadcrumb> {
        let buf = self.breadcrumbs.lock().unwrap_or_else(|e| e.into_inner());
        buf.iter().cloned().collect()
    }

    pub(crate) fn take_pending(&self) -> Vec<Event<'static>> {
        let mut q = self.pending.lock().unwrap_or_else(|e| e.into_inner());
        q.drain(..).collect()
    }

    fn push_pending(&self, event: Event<'static>) {
        let mut q = self.pending.lock().unwrap_or_else(|e| e.into_inner());
        q.push_back(event);
        while q.len() > MAX_PENDING {
            q.pop_front();
        }
    }
}

/// Sentry options wired to capture into `captured` and transmit nothing.
pub(crate) fn client_options(
    config: &Config,
    captured: Arc<Captured>,
) -> anyhow::Result<ClientOptions> {
    let dsn = config
        .dsn
        .parse::<sentry::types::Dsn>()
        .map_err(|err| anyhow::anyhow!("invalid Sentry DSN: {err}"))?;

    Ok(ClientOptions {
        dsn: Some(dsn),
        release: Some(config.release.clone().into()),
        environment: Some(config.environment.clone().into()),
        before_send: Some(stash_instead_of_sending(captured)),
        ..Default::default()
    })
}

/// The consent gate. Stamps the breadcrumb trail onto the event, stashes it, and
/// returns `None` so the SDK's own pipeline never reaches the transport — only a
/// user-initiated command sends anything.
fn stash_instead_of_sending(captured: Arc<Captured>) -> BeforeCallback<Event<'static>> {
    Arc::new(move |mut event| {
        event.breadcrumbs = captured.snapshot_breadcrumbs().into();
        captured.push_pending(event);
        None
    })
}
