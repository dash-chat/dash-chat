import { listen } from '@tauri-apps/api/event';
import { UnsubscribeFunction } from 'emittery';

import { ChatId, MessageAcks, SystemEvent } from '../types';
import { invokeAfterSetup } from '../utils/invoke-after-setup';

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
		let unsubs: (() => void) | undefined;
		listen('dashchat://system-event', e => {
			const event = e.payload as SystemEvent;
			if (event.type !== 'MessageAcks') return;
			if (event.payload.topic !== chatId) return;
			handler(event.payload.acks);
		}).then(u => (unsubs = u));

		return () => {
			if (unsubs) unsubs();
		};
	}
}
