import { reactive, relay } from 'signalium';

import type { LogsClient } from './logs-client';
import type { SimplifiedOperation } from './simplified-types';
import type { PublicKey, TopicId } from './types';

export class LogsStore<PAYLOAD> {
	constructor(public logsClient: LogsClient<PAYLOAD>) {}

	authorsForTopic = reactive((topicId: TopicId) =>
		relay<PublicKey[]>(state => {
			const fetchAuthors = async () => {
				const authors = await this.logsClient.getAuthorsForTopic(topicId);
				state.value = authors;
			};
			fetchAuthors();

			const unsubs = this.logsClient.onNewOperation(
				(operationTopicId, operation) => {
					if (topicId !== operationTopicId) return;
					const authors = state.value || [];
					const author = operation.header.public_key;
					if (authors.includes(author)) return;
					state.value = [...(state.value || []), author];
				},
			);

			return unsubs;
		}),
	);

	logs = reactive((topicId: TopicId, author: PublicKey) =>
		relay<SimplifiedOperation<PAYLOAD>[]>(state => {
			const fetchLog = async () => {
				const log = await this.logsClient.getLog(topicId, author);
				state.value = log;
			};
			fetchLog();

			const unsubs = this.logsClient.onNewOperation(
				(operationTopicId, operation) => {
					if (topicId !== operationTopicId) return;
					if (author !== operation.header.public_key) return;

					// We already have this operation
					if (state.value?.find(op => op.header.seq_num === operation.header.seq_num)) return;

					state.value = [...(state.value || []), operation];
				},
			);
			return () => {
				unsubs();
			};
		}),
	);

	logsForAllAuthors = reactive(async (topicId: TopicId) => {
		const authorsForTopic = await this.authorsForTopic(topicId);

		const logs = await Promise.all(
			authorsForTopic.map(author => this.logs(topicId, author)),
		);

		const logsForAllAuthors: Record<PublicKey, SimplifiedOperation<PAYLOAD>[]> = {};
		for (let i = 0; i < authorsForTopic.length; i++) {
			logsForAllAuthors[authorsForTopic[i]] = logs[i];
		}

		return logsForAllAuthors;
	});
}
