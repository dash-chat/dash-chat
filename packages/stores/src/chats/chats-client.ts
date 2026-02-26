import { invoke } from '@tauri-apps/api/core';

import { Hash } from '../p2panda/types';
import { ChatId } from '../types';

export interface IChatsClient {
	createGroupChat(chatId: ChatId): Promise<void>;
	getGroupChats(): Promise<Array<ChatId>>;
	markMessagesRead(chatId: ChatId, messageHashes: Hash[]): Promise<void>;
}

export class ChatsClient implements IChatsClient {
	createGroupChat(groupChatId: ChatId): Promise<void> {
		return invoke('create_group_chat', {
			groupChatId,
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
