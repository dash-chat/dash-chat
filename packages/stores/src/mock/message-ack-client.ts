import type { UnsubscribeFunction } from 'emittery';

import type { IMessageAckClient } from '../message-acks/message-ack-client';
import type { ChatId, MessageAcks } from '../types';

/** Mock mode has no peers, so nothing is ever delivered. */
export class MockMessageAckClient implements IMessageAckClient {
	async getMessageAcks(_chatId: ChatId): Promise<MessageAcks> {
		return {};
	}

	onNewMessageAcks(
		_chatId: ChatId,
		_handler: (acks: MessageAcks) => void,
	): UnsubscribeFunction {
		return () => {};
	}
}
