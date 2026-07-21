import type { IMessagesClient } from '../chats/messages-client';
import type { Hash, TopicId } from '../p2panda/types';
import type { ChatId, ChatReaction, OutgoingMedia, Tombstone } from '../types';
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
		// The store passes the original message hash, but resolve the root
		// defensively (editChainHashes walks back to it) so the whole message is
		// named regardless of which version was pointed at.
		const chain = await this.editChainHashes(chatId, targetHash);
		const messageHash = chain[chain.length - 1];
		return this.logsClient.create(this.deviceGroupTopicId, {
			type: 'DeviceGroupPayload',
			payload: {
				type: 'DeleteForMe',
				payload: { chat_id: chatId, message_hash: messageHash },
			},
		});
	}

	async getTombstones(chatId: ChatId): Promise<Tombstone[]> {
		const chatOps = await this.allOps(chatId);
		const tombstones: Tombstone[] = [];

		// Delete-for-everyone: the DeleteMessage op already lists its whole chain.
		for (const op of chatOps) {
			const body = op.body;
			if (body?.type === 'Chat' && body.payload?.type === 'DeleteMessage') {
				for (const hash of body.payload.payload.hashes) {
					tombstones.push({ hash, reason: 'DeletedForEveryone' });
				}
			}
		}

		// target hash -> the edits pointing at it, for walking a message's chain.
		const editChildren: Record<Hash, Hash[]> = {};
		for (const op of chatOps) {
			const body = op.body;
			if (body?.type === 'Chat' && body.payload?.type === 'EditMessage') {
				const target = body.payload.payload.edit_hash;
				(editChildren[target] ??= []).push(op.hash);
			}
		}

		// Delete-for-me: each DeleteForMe names the original message; tombstone it
		// and everything reachable forward through the edit graph.
		const deviceGroupOps = await this.allOps(this.deviceGroupTopicId);
		for (const op of deviceGroupOps) {
			const body = op.body;
			if (
				body?.payload?.type === 'DeleteForMe' &&
				body.payload.payload.chat_id === chatId
			) {
				const root: Hash = body.payload.payload.message_hash;
				const closure = new Set<Hash>([root]);
				const pending = [root];
				while (pending.length > 0) {
					const hash = pending.pop() as Hash;
					for (const child of editChildren[hash] ?? []) {
						if (!closure.has(child)) {
							closure.add(child);
							pending.push(child);
						}
					}
				}
				closure.forEach(hash =>
					tombstones.push({ hash, reason: 'DeletedForMe' }),
				);
			}
		}

		return tombstones;
	}

	private async allOps(topicId: TopicId) {
		const authors = await this.logsClient.getAuthorsForTopic(topicId);
		const ops = [];
		for (const author of authors) {
			ops.push(...(await this.logsClient.getLog(topicId, author)));
		}
		return ops;
	}
}
