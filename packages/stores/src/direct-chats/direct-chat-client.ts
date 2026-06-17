import { invoke } from '@tauri-apps/api/core';

import { AgentId, Hash } from '../p2panda/types';
import { ChatId, ChatReaction, MessageContent } from '../types';

export interface IDirectChatClient {
	chatId(peer: AgentId): Promise<ChatId>;
	sendMessage(chatId: ChatId, content: MessageContent): Promise<Hash>;
	markMessagesRead(chatId: ChatId, messageHashes: Hash[]): Promise<void>;
	sendReaction(chatId: ChatId, content: ChatReaction): Promise<void>;
}

export class DirectChatClient implements IDirectChatClient {
	chatId(peer: AgentId): Promise<ChatId> {
		return invoke('direct_chat_id', {
			peer,
		});
	}

	async sendMessage(chatId: ChatId, content: MessageContent): Promise<Hash> {
		return invoke('send_message', {
			chatId,
			content,
		});
	}

	markMessagesRead(chatId: ChatId, messageHashes: Hash[]): Promise<void> {
		return invoke('mark_messages_read', {
			chatId,
			messageHashes,
		});
	}

	async sendReaction(chatId: ChatId, content: ChatReaction): Promise<void> {
		return invoke('send_reaction', {
			chatId,
			content,
		});
	}
}
