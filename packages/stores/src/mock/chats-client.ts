import type { IChatsClient } from '../chats/chats-client';
import type { PublicKey } from '../p2panda/types';
import type { Hash } from '../p2panda/types';
import type { ChatId } from '../types';

function random_hexadecimal(length: number) {
	let result = '';
	const characters = 'abcdef0123456789';
	for (let i = 0; i < length; i++)
		result += characters.charAt(Math.floor(Math.random() * characters.length));
	return result;
}

export class MockChatsClient implements IChatsClient {
	private groupChats: ChatId[] = [];

	async createGroup(_initialMembers: PublicKey[]): Promise<ChatId> {
		const chatId = random_hexadecimal(64);
		this.groupChats.push(chatId);
		return chatId;
	}

	async getGroupChats(): Promise<ChatId[]> {
		return this.groupChats;
	}

	async markMessagesRead(
		_chatId: ChatId,
		_messageHashes: Hash[],
	): Promise<void> {}
}
