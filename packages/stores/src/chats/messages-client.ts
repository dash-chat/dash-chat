import { Hash } from '../p2panda/types';
import { ChatId, ChatReaction, OutgoingMedia, Tombstone } from '../types';
import { invokeAfterSetup } from '../utils/invoke-after-setup';

export interface IMessagesClient {
	sendMessage(
		chatId: ChatId,
		message: string,
		media: OutgoingMedia | null,
		reply: Hash | null,
	): Promise<Hash>;
	markMessagesRead(chatId: ChatId, messageHashes: Hash[]): Promise<void>;
	sendReaction(chatId: ChatId, content: ChatReaction): Promise<void>;
	editMessage(chatId: ChatId, editHash: Hash, message: string): Promise<Hash>;
	deleteMessageForEveryone(chatId: ChatId, targetHash: Hash): Promise<Hash>;
	deleteMessageForMe(chatId: ChatId, targetHash: Hash): Promise<Hash>;
	getTombstones(chatId: ChatId): Promise<Tombstone[]>;
}

export class MessagesClient implements IMessagesClient {
	async sendMessage(
		chatId: ChatId,
		message: string,
		media: OutgoingMedia | null,
		reply: Hash | null,
	): Promise<Hash> {
		return invokeAfterSetup('send_message', {
			chatId,
			message,
			media,
			reply,
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

	editMessage(chatId: ChatId, editHash: Hash, message: string): Promise<Hash> {
		return invokeAfterSetup('edit_message', {
			chatId,
			editHash,
			message,
		});
	}

	deleteMessageForEveryone(chatId: ChatId, targetHash: Hash): Promise<Hash> {
		return invokeAfterSetup('delete_message', {
			chatId,
			targetHash,
		});
	}

	deleteMessageForMe(chatId: ChatId, targetHash: Hash): Promise<Hash> {
		return invokeAfterSetup('delete_message_for_me', {
			chatId,
			targetHash,
		});
	}

	getTombstones(chatId: ChatId): Promise<Tombstone[]> {
		return invokeAfterSetup('get_tombstones', { chatId });
	}
}
