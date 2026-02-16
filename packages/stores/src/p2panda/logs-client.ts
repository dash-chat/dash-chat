import type { UnsubscribeFunction } from 'emittery';

import type { SimplifiedOperation } from './simplified-types';
import type { PublicKey, TopicId } from './types';

export interface LogsClient<PAYLOAD> {
	getAuthorsForTopic(topicId: TopicId): Promise<PublicKey[]>;

	getLog(
		topicId: TopicId,
		author: PublicKey,
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
	filter: (operation: SimplifiedOperation<PAYLOAD>) => boolean,
): Promise<SimplifiedOperation<PAYLOAD>> {
	return new Promise((resolve, reject) => {
		const l = client.onNewOperation((opTopicId, op) => {
			if (!filter(op)) return;
			resolve(op);
			l();
		});
	});
}
