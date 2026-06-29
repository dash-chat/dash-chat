import { listen } from '@tauri-apps/api/event';
import { type UnsubscribeFunction } from 'emittery';

import { invokeAfterSetup } from '../utils/invoke-after-setup';
import type { LogsClient } from './logs-client';
import type { SimplifiedOperation } from './simplified-types';
import type { TopicId, VerifyingKey } from './types';

export class TauriLogsClient<PAYLOAD> implements LogsClient<PAYLOAD> {
	// myPubKey(): Promise<VerifyingKey> {
	// 	return invokeAfterSetup('my_pub_key');
	// }

	async getLog(
		topicId: TopicId,
		author: VerifyingKey,
	): Promise<SimplifiedOperation<PAYLOAD>[]> {
		return invokeAfterSetup('get_log', { topicId, author });
	}

	async getAuthorsForTopic(topicId: TopicId): Promise<VerifyingKey[]> {
		return invokeAfterSetup('get_authors', { topicId });
	}

	onNewOperation(
		handler: (
			topicId: TopicId,
			operation: SimplifiedOperation<PAYLOAD>,
		) => void,
	): UnsubscribeFunction {
		let unsubs: (() => void) | undefined;
		listen('p2panda://new-operation', e => {
			const operation = e.payload as SimplifiedOperation<PAYLOAD>;
			handler(operation.header.topic_id, operation);
		}).then(u => (unsubs = u));

		return () => {
			if (unsubs) unsubs();
		};
	}
}
