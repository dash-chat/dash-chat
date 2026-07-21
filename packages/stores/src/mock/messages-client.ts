import type { IMessagesClient } from '../chats/messages-client';
import type { Hash, TopicId } from '../p2panda/types';
import type { ChatId, ChatReaction, OutgoingMedia } from '../types';
import { type LocalStorageLogsClient } from './client';

export class MockMessagesClient implements IMessagesClient {
	constructor(
		private logsClient: LocalStorageLogsClient,
		private deviceGroupTopicId: TopicId,
	) {}

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

	async editMessage(
		chatId: ChatId,
		editHash: Hash,
		message: string,
	): Promise<Hash> {
		return this.logsClient.create(chatId, {
			type: 'Chat',
			payload: {
				type: 'EditMessage',
				payload: { message, edit_hash: editHash },
			},
		});
	}

	// Walk the edit chain from the target back to the original message so a
	// delete covers the whole chain, mirroring the backend.
	private async editChainHashes(
		chatId: ChatId,
		targetHash: Hash,
	): Promise<Hash[]> {
		const author = await this.logsClient.myPubKey();
		const log = await this.logsClient.getLog(chatId, author);
		const hashes = [targetHash];
		let current = targetHash;
		for (let i = 0; i < log.length; i++) {
			const op = log.find(o => o.hash === current);
			const body = op?.body;
			if (body?.type !== 'Chat' || body.payload.type !== 'EditMessage') break;
			current = body.payload.payload.edit_hash;
			hashes.push(current);
		}
		return hashes;
	}

	async deleteMessage(chatId: ChatId, targetHash: Hash): Promise<Hash> {
		const hashes = await this.editChainHashes(chatId, targetHash);
		return this.logsClient.create(chatId, {
			type: 'Chat',
			payload: { type: 'DeleteMessage', payload: { hashes } },
		});
	}

	async deleteMessageForMe(chatId: ChatId, targetHash: Hash): Promise<Hash> {
		const hashes = await this.editChainHashes(chatId, targetHash);
		return this.logsClient.create(this.deviceGroupTopicId, {
			type: 'DeviceGroupPayload',
			payload: { type: 'DeleteForMe', payload: { chat_id: chatId, hashes } },
		});
	}
}
