use sentry::protocol::{Event, Log, TraceContext};
use sentry::types::protocol::latest::TraceId;
use sentry::Envelope;

use crate::logs;
use crate::state::SentryState;

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

#[cfg(test)]
mod tests {
    use super::*;

    use sentry::protocol::{EnvelopeItem, ItemContainer};

    use crate::testing::{log_saying, plugin};

    #[test]
    fn a_report_carries_the_event_and_the_logs_on_its_trace() {
        let dir = tempfile::tempdir().unwrap();
        let (state, _) = plugin(dir.path());

        let envelope = build_envelope(&state, Event::default(), vec![log_saying("connecting")])
            .expect("before_send dropped the event");

        let event = envelope.event().expect("no event in the envelope");
        let sentry::protocol::Context::Trace(event_trace) = event
            .contexts
            .get("trace")
            .expect("no trace context on the event")
        else {
            panic!("the trace context is not a trace");
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
