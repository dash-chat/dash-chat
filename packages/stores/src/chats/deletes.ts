import type { Message } from './messages-store';
import type { SimplifiedOperation } from '../p2panda/simplified-types';
import type { DeviceId, Hash } from '../p2panda/types';
import type { Payload } from '../types';

/** The window during which a message may be deleted for everyone, measured
 * from the original message timestamp. Mirrors `DELETE_WINDOW_MICROS` in
 * `crates/dashchat-node/src/chat/delete.rs` (frontend timestamps are ms). */
export const DELETE_WINDOW_MS = 24 * 60 * 60 * 1000;

interface OpHeaderInfo {
	author: DeviceId;
	timestamp: number;
	seqNum: number;
}

/** Apply all deletes in `logs` to the already-built `messages` map, in place.
 *
 * Every message covered by a delete is replaced with a "deleted" placeholder
 * anchored at the chain's root operation — regardless of whether the deleted
 * payloads are still present locally: the backend tombstones them, and a
 * member that joined after the delete only ever sees body-less operations. A
 * delete only counts when authored by the same device as the operations it
 * covers, mirroring the backend's authorship rule. */
export function applyDeletes(
	messages: Record<Hash, Message>,
	logs: Record<DeviceId, SimplifiedOperation<Payload>[]>,
): void {
	const headers: Record<Hash, OpHeaderInfo> = {};
	const deletes: { author: DeviceId; hashes: Hash[] }[] = [];
	for (const operations of Object.values(logs)) {
		for (const op of operations) {
			headers[op.hash] = {
				author: op.header.verifying_key,
				timestamp: op.header.timestamp,
				seqNum: op.header.seq_num,
			};
			const body = op.body;
			if (body?.type === 'Chat' && body.payload.type === 'DeleteMessage') {
				deletes.push({
					author: op.header.verifying_key,
					hashes: body.payload.payload.hashes,
				});
			}
		}
	}

	for (const del of deletes) {
		const members = del.hashes
			.map(hash => ({ hash, header: headers[hash] }))
			.filter(m => m.header !== undefined);
		if (members.length === 0) continue;
		if (members.some(m => m.header.author !== del.author)) continue;

		// The whole chain lives in its author's log, published in order, so
		// the original message is the member with the lowest seq number.
		let root = members[0];
		for (const m of members) {
			if (m.header.seqNum < root.header.seqNum) root = m;
		}

		for (const { hash } of members) {
			delete messages[hash];
		}
		messages[root.hash] = {
			hash: root.hash,
			content: { message: '', media: null },
			author: root.header.author,
			seqNum: root.header.seqNum,
			timestamp: root.header.timestamp,
			reactions: {},
			deleted: true,
		};
	}
}
