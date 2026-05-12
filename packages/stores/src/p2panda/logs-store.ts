import { type ReactivePromise, reactive, relay } from 'signalium';

import type { LogsClient } from './logs-client';
import type { SimplifiedOperation } from './simplified-types';
import type { PublicKey, TopicId } from './types';

/// Stopgap: re-fetch each subscribed log on this interval as a safety net for
/// `p2panda://new-operation` events that don't reach this process. Concretely,
/// when an iOS push notification arrives in foreground, the NSE writes
/// operations to the shared `op_store` but the main app's notification channel
/// never fires for them — UI would otherwise stay stale until a manual reload.
/// Polling guarantees eventual consistency at the cost of one Tauri call per
/// active log per interval. Replace with cross-process change detection on
/// `op_store` (SQLite WAL + `PRAGMA data_version`) when that lands.
///
/// Only iOS is affected; on other platforms the channel events arrive
/// reliably, so we skip the polling there.
const POLL_INTERVAL_MS = 1_000;
const POLLING_ENABLED =
	typeof navigator !== 'undefined' &&
	(/iPhone|iPad|iPod/i.test(navigator.userAgent) ||
		// iPadOS 13+ reports a Mac user agent in WKWebView; fall back to the
		// touch-points heuristic so iPad users still get the polling safety net.
		(/Macintosh/i.test(navigator.userAgent) && navigator.maxTouchPoints > 1));

export class LogsStore<PAYLOAD> {
	constructor(public logsClient: LogsClient<PAYLOAD>) {}

	authorsForTopic: (topicId: TopicId) => ReactivePromise<PublicKey[]> = reactive(
		(topicId: TopicId) =>
			relay<PublicKey[]>(state => {
			const fetchAuthors = async () => {
				const authors = await this.logsClient.getAuthorsForTopic(topicId);
				const current = state.value;
				if (
					current &&
					current.length === authors.length &&
					authors.every(a => current.includes(a))
				)
					return;
				state.value = authors;
			};
			fetchAuthors();
			const interval = POLLING_ENABLED
				? setInterval(fetchAuthors, POLL_INTERVAL_MS)
				: undefined;

			const unsubs = this.logsClient.onNewOperation(
				(operationTopicId, operation) => {
					if (topicId !== operationTopicId) return;
					const authors = state.value || [];
					const author = operation.header.public_key;
					if (authors.includes(author)) return;
					state.value = [...(state.value || []), author];
				},
			);

			return () => {
				if (interval !== undefined) clearInterval(interval);
				unsubs();
			};
		}),
	);

	logs: (
		topicId: TopicId,
		author: PublicKey,
	) => ReactivePromise<SimplifiedOperation<PAYLOAD>[]> = reactive(
		(topicId: TopicId, author: PublicKey) =>
			relay<SimplifiedOperation<PAYLOAD>[]>(state => {
			const fetchLog = async () => {
				const log = await this.logsClient.getLog(topicId, author);
				const current = state.value;
				// Logs are append-only per author; same length means same content.
				if (current && current.length === log.length) return;
				state.value = log;
			};
			fetchLog();
			const interval = POLLING_ENABLED
				? setInterval(fetchLog, POLL_INTERVAL_MS)
				: undefined;

			const unsubs = this.logsClient.onNewOperation(
				(operationTopicId, operation) => {
					if (topicId !== operationTopicId) return;
					if (author !== operation.header.public_key) return;

					// We already have this operation
					if (
						state.value?.find(
							op => op.header.seq_num === operation.header.seq_num,
						)
					)
						return;

					state.value = [...(state.value || []), operation];
				},
			);
			return () => {
				if (interval !== undefined) clearInterval(interval);
				unsubs();
			};
		}),
	);

	logsForAllAuthors = reactive(async (topicId: TopicId) => {
		const authorsForTopic = await this.authorsForTopic(topicId);

		const logs = await Promise.all(
			authorsForTopic.map(author => this.logs(topicId, author)),
		);

		const logsForAllAuthors: Record<PublicKey, SimplifiedOperation<PAYLOAD>[]> =
			{};
		for (let i = 0; i < authorsForTopic.length; i++) {
			logsForAllAuthors[authorsForTopic[i]] = logs[i];
		}

		return logsForAllAuthors;
	});
}
