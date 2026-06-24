import { invoke } from '@tauri-apps/api/core';

import { AgentId, Hash } from '../p2panda/types';
import { ChatId, ChatReaction, OutgoingMedia } from '../types';

export interface IDirectChatClient {
	chatId(peer: AgentId): Promise<ChatId>;
	sendMessage(
		chatId: ChatId,
		message: string,
		media: OutgoingMedia | null,
	): Promise<Hash>;
	markMessagesRead(chatId: ChatId, messageHashes: Hash[]): Promise<void>;
	sendReaction(chatId: ChatId, content: ChatReaction): Promise<void>;
}

export class DirectChatClient implements IDirectChatClient {
	chatId(peer: AgentId): Promise<ChatId> {
		return invoke('direct_chat_id', {
			peer,
		});
	}

	async sendMessage(
		chatId: ChatId,
		message: string,
		media: OutgoingMedia | null,
	): Promise<Hash> {
		return invoke('send_message', {
			chatId,
			message,
			media,
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
