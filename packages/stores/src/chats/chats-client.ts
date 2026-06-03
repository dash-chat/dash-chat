import { invoke } from '@tauri-apps/api/core';

import { Hash, PublicKey } from '../p2panda/types';
import { ChatId } from '../types';

export interface IChatsClient {
	createGroup(initialMembers: PublicKey[]): Promise<ChatId>;
	getGroupChats(): Promise<Array<ChatId>>;
	markMessagesRead(chatId: ChatId, messageHashes: Hash[]): Promise<void>;
}

export class ChatsClient implements IChatsClient {
	createGroup(initialMembers: PublicKey[]): Promise<ChatId> {
		return invoke('create_group', {
			initialMembers,
		});
	}

	getGroupChats(): Promise<Array<ChatId>> {
		return invoke('get_group_chats');
	}

	markMessagesRead(chatId: ChatId, messageHashes: Hash[]): Promise<void> {
		return invoke('mark_messages_read', {
			chatId,
			messageHashes,
		});
	}
}
