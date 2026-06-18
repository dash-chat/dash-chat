import type { UnsubscribeFunction } from 'emittery';

import type { SimplifiedOperation } from './simplified-types';
import type { Hash, TopicId, VerifyingKey } from './types';

export interface LogsClient<PAYLOAD> {
	getAuthorsForTopic(topicId: TopicId): Promise<VerifyingKey[]>;

	getLog(
		topicId: TopicId,
		author: VerifyingKey,
	): Promise<SimplifiedOperation<PAYLOAD>[]>;

	onNewOperation(
		handler: (
			topicId: TopicId,
			operation: SimplifiedOperation<PAYLOAD>,
		) => void,
	): UnsubscribeFunction;
}

export async function waitForOperation<PAYLOAD>(
	client: LogsClient<PAYLOAD>,
	filter: (
		operation: SimplifiedOperation<PAYLOAD>,
		topicId: TopicId,
	) => boolean,
	timeout = 30_000,
): Promise<SimplifiedOperation<PAYLOAD>> {
	return new Promise((resolve, reject) => {
		const timer = setTimeout(() => {
			unsub();
			reject(new Error('waitForOperation timed out'));
		}, timeout);
		const unsub = client.onNewOperation((topicId, op) => {
			if (!filter(op, topicId)) return;
			clearTimeout(timer);
			unsub();
			resolve(op);
		});
	});
}
