import type { UnsubscribeFunction } from 'emittery';

import type { IMessageAckClient } from '../message-acks/message-ack-client';
import type { ChatId, MessageAcks } from '../types';
import { mockBackend } from './backend-state';

export class MockMessageAckClient implements IMessageAckClient {
	async getMessageAcks(_chatId: ChatId): Promise<MessageAcks> {
		return mockBackend.messageAcks;
	}

	onNewMessageAcks(
		_chatId: ChatId,
		_handler: (acks: MessageAcks) => void,
	): UnsubscribeFunction {
		return () => {};
	}
}
