import { SimplifiedOperation } from '../p2panda/simplified-types';
import { Hash } from '../p2panda/types';
import { Payload } from '../types';

/**
 * A direct chat both sides requested at once is spread over two spaces until
 * the side whose space lost the tie has moved its messages into the other
 * one. Until the losing space is given up, each moved message exists twice.
 */

/** The copy standing in for each original that was moved, by the original's
 * hash. A copy only counts for a message of its own author. */
export function movedMessages(
	ops: SimplifiedOperation<Payload>[],
): Record<Hash, SimplifiedOperation<Payload>> {
	const byHash: Record<Hash, SimplifiedOperation<Payload>> = {};
	for (const op of ops) byHash[op.hash] = op;

	const copies: Record<Hash, SimplifiedOperation<Payload>> = {};
	for (const op of ops) {
		const moved = movedFrom(op);
		if (moved === undefined) continue;
		const original = byHash[moved.hash];
		if (
			original !== undefined &&
			original.header.verifying_key !== op.header.verifying_key
		)
			continue;
		copies[moved.hash] = op;
	}
	return copies;
}

/** Shows each moved message once: its copy, at the time the original was
 * written, with whatever referred to the original referring to the copy. */
export function foldMovedMessages(
	ops: SimplifiedOperation<Payload>[],
): SimplifiedOperation<Payload>[] {
	const copies = movedMessages(ops);
	if (Object.keys(copies).length === 0) return ops;

	const byHash: Record<Hash, SimplifiedOperation<Payload>> = {};
	for (const op of ops) byHash[op.hash] = op;
	const copyHashes = new Set(Object.values(copies).map(copy => copy.hash));

	return ops
		.filter(op => copies[op.hash] === undefined)
		.map(op =>
			copyHashes.has(op.hash)
				? atOriginalTime(op, byHash)
				: retarget(op, copies),
		)
		.sort(
			(a, b) =>
				a.header.timestamp - b.header.timestamp || a.hash.localeCompare(b.hash),
		);
}

function movedFrom(op: SimplifiedOperation<Payload>) {
	if (op.body?.type !== 'Chat' || op.body.payload.type !== 'Message')
		return undefined;
	return op.body.payload.payload.moved ?? undefined;
}

/** The original's own timestamp when we still hold it, since a copy could
 * claim any; a claimed time is at least kept from postdating the copy. */
function atOriginalTime(
	copy: SimplifiedOperation<Payload>,
	byHash: Record<Hash, SimplifiedOperation<Payload>>,
): SimplifiedOperation<Payload> {
	const moved = movedFrom(copy);
	if (moved === undefined) return copy;
	const original = byHash[moved.hash];
	const claimed = Math.min(
		Math.floor(moved.timestamp / 1000),
		copy.header.timestamp,
	);
	return {
		...copy,
		header: {
			...copy.header,
			timestamp: original?.header.timestamp ?? claimed,
		},
	};
}

function retarget(
	op: SimplifiedOperation<Payload>,
	copies: Record<Hash, SimplifiedOperation<Payload>>,
): SimplifiedOperation<Payload> {
	const body = op.body;
	if (body?.type !== 'Chat') return op;
	const chat = body.payload;
	if (chat.type === 'Message') {
		const copy = chat.payload.reply ? copies[chat.payload.reply] : undefined;
		if (copy === undefined) return op;
		const payload = { ...chat.payload, reply: copy.hash };
		return { ...op, body: { ...body, payload: { ...chat, payload } } };
	}
	if (chat.type === 'Reaction') {
		const copy = copies[chat.payload.target];
		if (copy === undefined) return op;
		const payload = { ...chat.payload, target: copy.hash };
		return { ...op, body: { ...body, payload: { ...chat, payload } } };
	}
	if (chat.type === 'EditMessage') {
		const copy = copies[chat.payload.edit_hash];
		if (copy === undefined) return op;
		const payload = { ...chat.payload, edit_hash: copy.hash };
		return { ...op, body: { ...body, payload: { ...chat, payload } } };
	}
	return op;
}
