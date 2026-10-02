import { UnsubscribeFunction } from 'emittery';

import { ChatId, MessageAcks } from '../types';
import { invokeAfterSetup } from '../utils/invoke-after-setup';
import { onSystemEvent } from '../utils/system-events';

export interface IMessageAckClient {
	getMessageAcks(chatId: ChatId): Promise<MessageAcks>;
	onNewMessageAcks(
		chatId: ChatId,
		handler: (acks: MessageAcks) => void,
	): UnsubscribeFunction;
}

export class MessageAckClient implements IMessageAckClient {
	getMessageAcks(chatId: ChatId): Promise<MessageAcks> {
		return invokeAfterSetup('get_message_acks', { chatId });
	}

	onNewMessageAcks(
		chatId: ChatId,
		handler: (acks: MessageAcks) => void,
	): UnsubscribeFunction {
		return onSystemEvent('MessageAcks', event => {
			if (event.payload.topic === chatId) handler(event.payload.acks);
		});
	}
}
