use std::sync::Arc;

use sentry::integrations::contexts::utils::{device_context, os_context, rust_context};
use sentry::protocol::Event;
use sentry::{Client, ClientOptions};

use crate::Config;

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
pub(crate) fn enrich(mut event: Event<'static>, client: &Client) -> Event<'static> {
    let options = client.options();
    event.release.clone_from(&options.release);
    event.environment.clone_from(&options.environment);
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

        let event = enrich(Event::default(), &client);

        assert_eq!(event.server_name, None);
        assert_eq!(event.platform, "native");
        assert_eq!(event.release.as_deref(), Some("dash-chat@0.0.0"));
        assert_eq!(event.environment.as_deref(), Some("test"));
        for context in ["os", "rust", "device"] {
            assert!(event.contexts.contains_key(context), "missing {context}");
        }
    }
}
