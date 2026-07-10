import { AgentId, Hash } from '../p2panda/types';
import { ChatId, ChatReaction, OutgoingMedia } from '../types';
import { invokeAfterSetup } from '../utils/invoke-after-setup';

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
		return invokeAfterSetup('direct_chat_id', {
			peer,
		});
	}

	async sendMessage(
		chatId: ChatId,
		message: string,
		media: OutgoingMedia | null,
	): Promise<Hash> {
		return invokeAfterSetup('send_message', {
			chatId,
			message,
			media,
		});
	}

	markMessagesRead(chatId: ChatId, messageHashes: Hash[]): Promise<void> {
		return invokeAfterSetup('mark_messages_read', {
			chatId,
			messageHashes,
		});
	}

	async sendReaction(chatId: ChatId, content: ChatReaction): Promise<void> {
		return invokeAfterSetup('send_reaction', {
			chatId,
			content,
		});
	}
}
