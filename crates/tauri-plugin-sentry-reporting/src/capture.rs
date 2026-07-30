use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard};

use sentry::integrations::contexts::utils::{device_context, os_context, rust_context};
use sentry::protocol::{Breadcrumb, Event};
use sentry::{Client, ClientOptions};

use crate::Config;

const MAX_BREADCRUMBS: usize = 300;

/// The recent log trail, stamped onto a report as it goes out.
///
/// It lives here rather than on sentry's scope because a `Hub` is thread-local:
/// crumbs logged on a tokio worker would be invisible to a report assembled on a
/// Tauri command thread.
pub(crate) struct Breadcrumbs(Mutex<VecDeque<Breadcrumb>>);

impl Breadcrumbs {
    pub(crate) fn new() -> Self {
        Self(Mutex::new(VecDeque::new()))
    }

    pub(crate) fn add(&self, crumb: Breadcrumb) {
        let mut buf = self.lock();
        buf.push_back(crumb);
        while buf.len() > MAX_BREADCRUMBS {
            buf.pop_front();
        }
    }

    fn snapshot(&self) -> Vec<Breadcrumb> {
        self.lock().iter().cloned().collect()
    }

    fn lock(&self) -> MutexGuard<'_, VecDeque<Breadcrumb>> {
        self.0.lock().unwrap_or_else(|err| err.into_inner())
    }
}

/// Sentry options that cannot transmit anything on their own.
///
/// `before_send` rejects unconditionally, so everything the SDK captures by
/// itself — panics above all — is dropped. A report leaves through
/// [`Client::send_envelope`], which does not consult it.
pub(crate) fn client_options(config: &Config) -> anyhow::Result<ClientOptions> {
    let dsn = config
        .dsn
        .parse::<sentry::types::Dsn>()
        .map_err(|err| anyhow::anyhow!("invalid Sentry DSN: {err}"))?;

    Ok(ClientOptions {
        dsn: Some(dsn),
        release: Some(config.release.clone().into()),
        environment: Some(config.environment.clone().into()),
        before_send: Some(Arc::new(|_| None)),
        ..Default::default()
    })
}

/// Adds what sentry's own pipeline would have, minus anything naming the
/// machine: its `prepare_event` fills `server_name` from the OS hostname, which
/// on a personal machine is often the owner's real name.
pub(crate) fn enrich(
    mut event: Event<'static>,
    client: &Client,
    breadcrumbs: &Breadcrumbs,
) -> Event<'static> {
    let options = client.options();
    event.release.clone_from(&options.release);
    event.environment.clone_from(&options.environment);
    event.breadcrumbs = breadcrumbs.snapshot().into();
    // Sentry picks how to render the issue from this; the default is "other".
    event.platform = "native".into();

    if let Some(os) = os_context() {
        event.contexts.entry("os".into()).or_insert(os);
    }
    event
        .contexts
        .entry("rust".into())
        .or_insert_with(rust_context);
    event
        .contexts
        .entry("device".into())
        .or_insert_with(device_context);

    event
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> Config {
        Config {
            dsn: "https://key@example.invalid/1".into(),
            release: "dash-chat@0.0.0".into(),
            environment: "test".into(),
            redact: vec![],
        }
    }

    #[test]
    fn the_sdk_can_never_transmit_on_its_own() {
        let before_send = client_options(&config()).unwrap().before_send.unwrap();

        assert!(before_send(Event::default()).is_none());
    }

    #[test]
    fn enriching_adds_context_but_never_the_hostname() {
        let client = Client::from(client_options(&config()).unwrap());
        let breadcrumbs = Breadcrumbs::new();
        breadcrumbs.add(Breadcrumb {
            message: Some("connecting".into()),
            ..Default::default()
        });

        let event = enrich(Event::default(), &client, &breadcrumbs);

        assert_eq!(event.server_name, None);
        assert_eq!(event.platform, "native");
        assert_eq!(event.release.as_deref(), Some("dash-chat@0.0.0"));
        assert_eq!(event.environment.as_deref(), Some("test"));
        assert_eq!(event.breadcrumbs.len(), 1);
        for context in ["os", "rust", "device"] {
            assert!(event.contexts.contains_key(context), "missing {context}");
        }
    }
}
