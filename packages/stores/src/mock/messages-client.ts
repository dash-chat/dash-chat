import type { IMessagesClient } from '../chats/messages-client';
import type { Hash } from '../p2panda/types';
import type { ChatId, ChatReaction, OutgoingMedia } from '../types';
import { type LocalStorageLogsClient } from './client';

export class MockMessagesClient implements IMessagesClient {
	constructor(private logsClient: LocalStorageLogsClient) {}

	async sendMessage(
		chatId: ChatId,
		message: string,
		_media: OutgoingMedia | null,
	): Promise<Hash> {
		return this.logsClient.create(chatId, {
			type: 'Chat',
			payload: {
				type: 'Message',
				payload: { v: '1', message, media: null },
			},
		});
	}

	async markMessagesRead(
		_chatId: ChatId,
		_messageHashes: Hash[],
	): Promise<void> {}

	async sendReaction(chatId: ChatId, content: ChatReaction): Promise<void> {
		await this.logsClient.create(chatId, {
			type: 'Chat',
			payload: { type: 'Reaction', payload: content },
		});
	}
}
