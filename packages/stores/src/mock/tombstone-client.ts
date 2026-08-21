import type { UnsubscribeFunction } from 'emittery';

import type { SimplifiedOperation } from '../p2panda/simplified-types';
import type { Hash, TopicId } from '../p2panda/types';
import type { ITombstoneClient } from '../tombstones/tombstone-client';
import type { ChatId, Payload, Tombstone, Tombstones } from '../types';
import { type LocalStorageLogsClient } from './client';

type Op = SimplifiedOperation<Payload>;

export class MockTombstoneClient implements ITombstoneClient {
	constructor(
		private logsClient: LocalStorageLogsClient,
		private deviceGroupTopicId: TopicId,
	) {}

	async getTombstones(chatId: ChatId): Promise<Tombstones> {
		const chatOps = await this.allOps(chatId);
		const deviceGroupOps = await this.allOps(this.deviceGroupTopicId);
		const tombstones: Tombstones = {};
		for (const tombstone of [
			...deletedForEveryoneTombstones(chatOps),
			...deletedForMeTombstones(chatId, chatOps, deviceGroupOps),
		]) {
			tombstones[tombstone.hash] = tombstone.reason;
		}
		return tombstones;
	}

	onNewTombstones(
		chatId: ChatId,
		handler: (tombstone: Tombstone) => void,
	): UnsubscribeFunction {
		const seen = new Set<string>();
		// The store fetches the current tombstones itself, so record those without
		// emitting them and only push what shows up afterwards.
		const primed = this.getTombstones(chatId).then(tombstones =>
			Object.entries(tombstones).forEach(([hash, reason]) =>
				seen.add(tombstoneKey({ hash, reason })),
			),
		);

		const emitNew = async () => {
			await primed;
			for (const [hash, reason] of Object.entries(
				await this.getTombstones(chatId),
			)) {
				const key = tombstoneKey({ hash, reason });
				if (seen.has(key)) continue;
				seen.add(key);
				handler({ hash, reason });
			}
		};

		return this.logsClient.onNewOperation(topicId => {
			if (topicId !== chatId && topicId !== this.deviceGroupTopicId) return;
			emitNew();
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

function tombstoneKey(tombstone: Tombstone): string {
	return `${tombstone.hash}/${tombstone.reason}`;
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
