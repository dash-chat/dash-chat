import type { IDirectChatClient } from '../direct-chats/direct-chat-client';
import type { AgentId, Hash } from '../p2panda/types';
import type { ChatId, ChatReaction, OutgoingMedia } from '../types';
import { type LocalStorageLogsClient, hash } from './client';

export class MockDirectChatClient implements IDirectChatClient {
	constructor(
		private logsClient: LocalStorageLogsClient,
		private agentId: AgentId,
	) {}

	async chatId(peer: AgentId): Promise<ChatId> {
		return hash([this.agentId, peer].sort().join(':'));
	}

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

	async deleteMessage(chatId: ChatId, targetHash: Hash): Promise<Hash> {
		const author = await this.logsClient.myPubKey();
		const log = await this.logsClient.getLog(chatId, author);
		// Walk the edit chain from the target back to the original message so
		// the delete covers the whole chain, mirroring the backend.
		const hashes = [targetHash];
		let current = targetHash;
		for (;;) {
			const op = log.find(o => o.hash === current);
			const body = op?.body;
			if (body?.type !== 'Chat' || body.payload.type !== 'EditMessage') break;
			current = body.payload.payload.edit_hash;
			hashes.push(current);
		}
		return this.logsClient.create(chatId, {
			type: 'Chat',
			payload: { type: 'DeleteMessage', payload: { hashes } },
		});
	}
}
