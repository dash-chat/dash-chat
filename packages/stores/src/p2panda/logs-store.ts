import { type ReactivePromise, reactive, relay } from 'signalium';

import type { LogsClient } from './logs-client';
import type { SimplifiedOperation } from './simplified-types';
import type { TopicId, VerifyingKey } from './types';

export class LogsStore<PAYLOAD> {
	constructor(public logsClient: LogsClient<PAYLOAD>) {}

	authorsForTopic = reactive(
		(topicId: TopicId): ReactivePromise<VerifyingKey[]> =>
			relay<VerifyingKey[]>(state => {
				const fetchAuthors = async () => {
					try {
						const authors = await this.logsClient.getAuthorsForTopic(topicId);
						const current = state.value;

						let allAuthorsAreKnown = true;
						for (const author of authors) {
							if (!current?.includes(author)) {
								allAuthorsAreKnown = false;
							}
						}

						if (
							!current ||
							current.length !== authors.length ||
							!allAuthorsAreKnown
						) {
							state.value = authors;
						}
						return authors;
					} catch (error) {
						state.setError(error);
					}
				};
				fetchAuthors();

				const unsubs = this.logsClient.onNewOperation(
					(operationTopicId, operation) => {
						if (topicId !== operationTopicId) return;
						const authors = state.value || [];
						const author = operation.header.verifying_key;
						if (authors.includes(author)) return;
						state.value = [...(state.value || []), author];
					},
				);

				return () => unsubs();
			}),
	);

	logs = reactive(
		(
			topicId: TopicId,
			author: VerifyingKey,
		): ReactivePromise<SimplifiedOperation<PAYLOAD>[]> =>
			relay<SimplifiedOperation<PAYLOAD>[]>(state => {
				const fetchLog = async () => {
					try {
						const log = await this.logsClient.getLog(topicId, author);
						const current = state.value;
						// Logs are append-only per author; same length means same content.
						if (!(current && current.length === log.length)) {
							state.value = log;
						}
						return log;
					} catch (error) {
						state.setError(error);
					}
				};
				fetchLog();

				const unsubs = this.logsClient.onNewOperation(
					(operationTopicId, operation) => {
						if (topicId !== operationTopicId) return;
						if (author !== operation.header.verifying_key) return;

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
				return () => unsubs();
			}),
	);

	logsForAllAuthors = reactive(async (topicId: TopicId) => {
		const authorsForTopic = await this.authorsForTopic(topicId);

		const logs = await Promise.all(
			authorsForTopic.map(author => this.logs(topicId, author)),
		);

		const logsForAllAuthors: Record<
			VerifyingKey,
			SimplifiedOperation<PAYLOAD>[]
		> = {};
		for (let i = 0; i < authorsForTopic.length; i++) {
			logsForAllAuthors[authorsForTopic[i]] = logs[i];
		}

		return logsForAllAuthors;
	});
}
