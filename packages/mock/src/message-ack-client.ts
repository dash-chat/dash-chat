import type { ChatId, IMessageAckClient, MessageAcks } from 'dash-chat-stores';
import type { UnsubscribeFunction } from 'emittery';

import { mockBackend, onMockBackendChanged } from './backend-state';

export class MockMessageAckClient implements IMessageAckClient {
	async getMessageAcks(_chatId: ChatId): Promise<MessageAcks> {
		return mockBackend().messageAcks;
	}

	onNewMessageAcks(
		_chatId: ChatId,
		handler: (acks: MessageAcks) => void,
	): UnsubscribeFunction {
		return onMockBackendChanged(() => handler(mockBackend().messageAcks));
	}
}
