import type {
	ChatId,
	Hash,
	IChatsClient,
	VerifyingKey,
} from 'dash-chat-stores';
import type { UnsubscribeFunction } from 'emittery';

import { failIfTold } from './backend-state';
import {
	mockGroupChatIds,
	onMockGroupChatAdded,
	setMockGroupMembers,
} from './group-chats';

function random_hexadecimal(length: number) {
	let result = '';
	const characters = 'abcdef0123456789';
	for (let i = 0; i < length; i++)
		result += characters.charAt(Math.floor(Math.random() * characters.length));
	return result;
}

export class MockChatsClient implements IChatsClient {
	async createGroup(_initialMembers: VerifyingKey[]): Promise<ChatId> {
		const chatId = random_hexadecimal(64);
		setMockGroupMembers(chatId, []);
		return chatId;
	}

	onGroupChatAdded(handler: (chatId: ChatId) => void): UnsubscribeFunction {
		return onMockGroupChatAdded(handler);
	}

	async getGroupChats(): Promise<ChatId[]> {
		failIfTold('get_group_chats');
		return mockGroupChatIds();
	}

	async markMessagesRead(
		_chatId: ChatId,
		_messageHashes: Hash[],
	): Promise<void> {}
}
