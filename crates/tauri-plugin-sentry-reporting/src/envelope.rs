use sentry::protocol::{EnvelopeItem, Event, Log, TraceContext};
use sentry::types::protocol::latest::TraceId;
use sentry::Envelope;

use crate::state::SentryState;
use crate::{attachment, logs};

pub(crate) fn build_envelope(
    state: &SentryState,
    mut event: Event<'static>,
    logs: Vec<Log>,
) -> Option<Envelope> {
    let trace_id = TraceId::default();
    event.contexts.insert(
        "trace".into(),
        TraceContext {
            trace_id,
            ..Default::default()
        }
        .into(),
    );

    let event = state.client.prepare_event(event, None)?;

    let mut envelope: Envelope = event.into();
    if !logs.is_empty() {
        envelope.add_item(logs::envelope_item(logs, trace_id));
    }
    Some(envelope)
}

/// The only path to the network. The logs and the log file overlap on purpose:
/// the logs are searchable but recent, the file reaches further back.
pub(crate) async fn send(state: &SentryState, mut envelope: Envelope) {
    if let Some(attachment) = attachment::build_logs_attachment(state).await {
        envelope.add_item(EnvelopeItem::Attachment(attachment));
    }
    state.gate.send(envelope);
}

#[cfg(test)]
mod tests {
    use super::*;

    use sentry::protocol::ItemContainer;

    use crate::testing::{log_saying, plugin};

    #[test]
    fn a_report_carries_the_event_the_logs_and_the_log_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("app.log"), "a line\n").unwrap();
        let (state, recorder) = plugin(dir.path());

        let envelope = build_envelope(&state, Event::default(), vec![log_saying("connecting")])
            .expect("before_send dropped the event");
        tauri::async_runtime::block_on(send(&state, envelope));

        let envelope = recorder.only();
        assert!(envelope.event().is_some());
        assert!(envelope
            .items()
            .any(|item| matches!(item, EnvelopeItem::ItemContainer(ItemContainer::Logs(_)))));
        assert!(envelope
            .items()
            .any(|item| matches!(item, EnvelopeItem::Attachment(_))));
    }

    #[test]
    fn the_logs_share_the_events_trace() {
        let dir = tempfile::tempdir().unwrap();
        let (state, recorder) = plugin(dir.path());

        let envelope = build_envelope(&state, Event::default(), vec![log_saying("connecting")])
            .expect("before_send dropped the event");
        tauri::async_runtime::block_on(send(&state, envelope));

        let envelope = recorder.only();
        let event_trace = envelope.event().unwrap().contexts.get("trace").unwrap();
        let sentry::protocol::Context::Trace(event_trace) = event_trace else {
            panic!("no trace context on the event");
        };
        let logs = envelope
            .items()
            .find_map(|item| match item {
                EnvelopeItem::ItemContainer(ItemContainer::Logs(logs)) => Some(logs),
                _ => None,
            })
            .expect("no logs container");
        assert!(logs
            .iter()
            .all(|log| log.trace_id == Some(event_trace.trace_id)));
    }
}
