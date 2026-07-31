use std::sync::Arc;

use sentry::integrations::backtrace::ProcessStacktraceIntegration;
use sentry::integrations::contexts::ContextIntegration;
use sentry::integrations::debug_images::DebugImagesIntegration;
use sentry::ClientOptions;

use crate::logs::Pending;
use crate::redaction;
use crate::transport::{ConsentGate, ConsentGateFactory};
use crate::Config;

/// Sentry's own pipeline, minus everything that would transmit unasked.
pub(crate) fn options(
    config: &Config,
    pending: Arc<Pending>,
    gate: Arc<ConsentGate>,
) -> ClientOptions {
    let patterns = config.redact.clone();
    let log_patterns = config.redact.clone();

    let mut options = ClientOptions::new()
        .release(config.release.clone())
        .environment(config.environment.clone())
        // `ContextIntegration` fills this from the OS hostname when it is unset,
        // which on a personal machine is often the owner's real name.
        .server_name("")
        // Stacktraces are captured from inside a hook, so these frames sit above
        // the code that raised them; 0.49 dropped the trimming that cut them.
        .in_app_exclude([
            "tauri_plugin_sentry_reporting::",
            "sentry_panic::",
            "__rustc::",
        ])
        // `PanicIntegration` is the only default integration that captures by itself.
        .default_integrations(false)
        .add_integration(ProcessStacktraceIntegration)
        .add_integration(DebugImagesIntegration::default())
        .add_integration(ContextIntegration::default())
        // The last step of `prepare_event`, so nothing escapes unredacted.
        .before_send(move |mut event| {
            event.server_name = None;
            redaction::redact_serialized(&patterns, event)
                .inspect_err(|err| log::error!("sentry-reporting: dropping unredactable: {err}"))
                .ok()
        })
        // Without this `capture_log` never reaches `before_send_log`.
        .enable_logs(true)
        // `None` keeps the batcher from ever being handed a log, so it never
        // flushes on its own; a report carries what was kept here instead.
        .before_send_log(move |log| {
            match redaction::redact_serialized(&log_patterns, log) {
                Ok(log) => pending.push(log),
                // Not `log::error!`: this is reached from inside the logger.
                Err(err) => eprintln!("sentry-reporting: dropping unredactable log: {err}"),
            }
            None
        })
        .transport(ConsentGateFactory(gate));
    // `ClientOptions::dsn` takes a `&str` and panics on a bad parse.
    options.dsn = Some(config.dsn.clone());
    options
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::Path;

    use sentry::protocol::Event;
    use sentry::Client;

    use crate::testing::{config, log_saying, recording_gate};

    fn options_keeping(pending: Arc<Pending>) -> ClientOptions {
        let (gate, _) = recording_gate();
        options(&config(Path::new("")), pending, Arc::new(gate))
    }

    fn prepared(event: Event<'static>) -> Event<'static> {
        Client::from(options_keeping(Arc::new(Pending::default())))
            .prepare_event(event, None)
            .expect("before_send dropped the event")
    }

    #[test]
    fn no_installed_integration_captures_on_its_own() {
        let options = options_keeping(Arc::new(Pending::default()));

        assert!(!options.default_integrations);
        let names: Vec<_> = options.integrations.iter().map(|i| i.name()).collect();
        assert_eq!(names, ["process-stacktrace", "debug-images", "contexts"]);
    }

    #[test]
    fn a_captured_log_is_kept_rather_than_queued_for_sending() {
        let pending = Arc::new(Pending::default());
        let client = Client::from(options_keeping(pending.clone()));

        client.capture_log(log_saying("connecting"), &Default::default());

        assert_eq!(pending.snapshot().len(), 1);
    }

    #[test]
    fn a_kept_log_is_already_redacted() {
        let pending = Arc::new(Pending::default());
        let client = Client::from(options_keeping(pending.clone()));

        client.capture_log(log_saying("token secret-abc123"), &Default::default());

        assert_eq!(pending.snapshot()[0].body, "token [REDACTED]");
    }

    #[test]
    fn preparing_adds_context_but_never_the_hostname() {
        let event = prepared(Event::default());

        assert_eq!(event.server_name, None);
        assert_eq!(event.platform, "native");
        assert_eq!(event.release.as_deref(), Some("dash-chat@0.0.0"));
        assert_eq!(event.environment.as_deref(), Some("test"));
        for context in ["os", "rust", "device"] {
            assert!(event.contexts.contains_key(context), "missing {context}");
        }
    }

    #[test]
    fn preparing_redacts_the_event() {
        let event = prepared(Event {
            message: Some("failed for secret-abc123".into()),
            ..Default::default()
        });

        assert_eq!(event.message.as_deref(), Some("failed for [REDACTED]"));
    }
}
