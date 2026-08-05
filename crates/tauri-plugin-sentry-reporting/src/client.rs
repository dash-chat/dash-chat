use std::sync::Arc;

use sentry::integrations::backtrace::ProcessStacktraceIntegration;
use sentry::integrations::contexts::ContextIntegration;
use sentry::integrations::debug_images::DebugImagesIntegration;
use sentry::ClientOptions;

use crate::logs::PendingLogs;
use crate::redaction;
use crate::transport::{UserInitiatedTransport, UserInitiatedTransportFactory};
use crate::Config;

/// We use Sentry's own pipeline, except we only transmit logs when the user
/// manually accepts to do so via UserInitiatedTransport::send()
pub(crate) fn options(
    config: &Config,
    pending: Arc<PendingLogs>,
    transport: Arc<UserInitiatedTransport>,
) -> ClientOptions {
    let patterns = config.redact.clone();
    let log_patterns = config.redact.clone();

    let mut options = ClientOptions::new()
        .release(config.release.clone())
        .environment(config.environment.clone())
        // `ContextIntegration` fills this from the OS hostname when it is unset,
        // which on a personal machine is often the owner's real name.
        .server_name("")
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
        .transport(UserInitiatedTransportFactory(transport));
    options.dsn = Some(config.dsn.clone());
    options
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::Path;

    use sentry::protocol::Event;
    use sentry::Client;

    use crate::testing::{config, log_saying, recording_transport};

    fn options_keeping(pending: Arc<PendingLogs>) -> ClientOptions {
        let config = config(Path::new(""));
        let (transport, _) = recording_transport(&config);
        options(&config, pending, Arc::new(transport))
    }

    #[test]
    fn a_captured_log_is_kept_redacted_rather_than_queued_for_sending() {
        let pending = Arc::new(PendingLogs::default());
        let client = Client::from(options_keeping(pending.clone()));

        client.capture_log(log_saying("token secret-abc123"), &Default::default());

        let kept = pending.snapshot();
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].body, "token [REDACTED]");
    }

    #[test]
    fn preparing_redacts_and_adds_context_but_never_the_hostname() {
        let event = Client::from(options_keeping(Arc::new(PendingLogs::default())))
            .prepare_event(
                Event {
                    message: Some("failed for secret-abc123".into()),
                    ..Default::default()
                },
                None,
            )
            .expect("before_send dropped the event");

        assert_eq!(event.message.as_deref(), Some("failed for [REDACTED]"));
        assert_eq!(event.server_name, None);
        assert_eq!(event.platform, "native");
        assert_eq!(event.release.as_deref(), Some("dash-chat@0.0.0"));
        assert_eq!(event.environment.as_deref(), Some("test"));
        for context in ["os", "rust", "device"] {
            assert!(event.contexts.contains_key(context), "missing {context}");
        }
    }
}
