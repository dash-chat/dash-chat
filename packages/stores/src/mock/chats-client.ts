import Emittery, { type UnsubscribeFunction } from 'emittery';

import type { IChatsClient } from '../chats/chats-client';
import type { VerifyingKey } from '../p2panda/types';
import type { Hash } from '../p2panda/types';
import type { ChatId } from '../types';
import { failIfTold } from './backend-state';
import { mockGroupChatIds, setMockGroupMembers } from './group-chats';

function random_hexadecimal(length: number) {
	let result = '';
	const characters = 'abcdef0123456789';
	for (let i = 0; i < length; i++)
		result += characters.charAt(Math.floor(Math.random() * characters.length));
	return result;
}

export class MockChatsClient implements IChatsClient {
	private emitter = new Emittery<{ 'group-chat-added': ChatId }>();

	async createGroup(_initialMembers: VerifyingKey[]): Promise<ChatId> {
		const chatId = random_hexadecimal(64);
		setMockGroupMembers(chatId, []);
		void this.emitter.emit('group-chat-added', chatId);
		return chatId;
	}

	onGroupChatAdded(handler: (chatId: ChatId) => void): UnsubscribeFunction {
		return this.emitter.on('group-chat-added', handler);
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
