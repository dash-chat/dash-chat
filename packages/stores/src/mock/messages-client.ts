import type { IMessagesClient } from '../chats/messages-client';
import type { SimplifiedOperation } from '../p2panda/simplified-types';
import type { Hash, TopicId } from '../p2panda/types';
import type {
	ChatId,
	ChatReaction,
	OutgoingMedia,
	Payload,
	Tombstone,
} from '../types';
import { type LocalStorageLogsClient } from './client';

type Op = SimplifiedOperation<Payload>;

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

	async deleteMessageForEveryone(
		chatId: ChatId,
		targetHash: Hash,
	): Promise<Hash> {
		const hashes = await this.editChainHashes(chatId, targetHash);
		return this.logsClient.create(chatId, {
			type: 'Chat',
			payload: { type: 'DeleteMessage', payload: { hashes } },
		});
	}

	async deleteMessageForMe(chatId: ChatId, targetHash: Hash): Promise<Hash> {
		// The store passes the original message hash, but resolve the root
		// defensively (editChainHashes walks back to it) so the whole message is
		// referenced regardless of which version was pointed at.
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

	private async allOps(topicId: TopicId): Promise<Op[]> {
		const authors = await this.logsClient.getAuthorsForTopic(topicId);
		const ops = [];
		for (const author of authors) {
			ops.push(...(await this.logsClient.getLog(topicId, author)));
		}
		return ops;
	}
}

/** A `DeleteMessage` already names its whole chain, so its hashes are the
 * tombstones. */
function deletedForEveryoneTombstones(chatOps: Op[]): Tombstone[] {
	const tombstones: Tombstone[] = [];
	for (const op of chatOps) {
		const body = op.body;
		if (body?.type === 'Chat' && body.payload.type === 'DeleteMessage') {
			for (const hash of body.payload.payload.hashes) {
				tombstones.push({ hash, reason: 'DeletedForEveryone' });
			}
		}
	}
	return tombstones;
}

/** A `DeleteForMe` names one message; it and every edit reachable forward from
 * it are tombstoned, mirroring the backend's `forward_edit_closure`. */
function deletedForMeTombstones(
	chatId: ChatId,
	chatOps: Op[],
	deviceGroupOps: Op[],
): Tombstone[] {
	const editChildren = editChildrenIndex(chatOps);
	const tombstones: Tombstone[] = [];
	for (const op of deviceGroupOps) {
		const body = op.body;
		if (body?.type !== 'DeviceGroupPayload') continue;
		if (body.payload.type !== 'DeleteForMe') continue;
		if (body.payload.payload.chat_id !== chatId) continue;
		for (const hash of forwardEditClosure(
			editChildren,
			body.payload.payload.message_hash,
		)) {
			tombstones.push({ hash, reason: 'DeletedForMe' });
		}
	}
	return tombstones;
}

/** Target hash -> the hashes of the edits pointing at it. */
function editChildrenIndex(chatOps: Op[]): Record<Hash, Hash[]> {
	const editChildren: Record<Hash, Hash[]> = {};
	for (const op of chatOps) {
		const body = op.body;
		if (body?.type === 'Chat' && body.payload.type === 'EditMessage') {
			(editChildren[body.payload.payload.edit_hash] ??= []).push(op.hash);
		}
	}
	return editChildren;
}

/** `root` plus every edit that transitively points at it. */
function forwardEditClosure(
	editChildren: Record<Hash, Hash[]>,
	root: Hash,
): Hash[] {
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
	return Array.from(closure);
}
