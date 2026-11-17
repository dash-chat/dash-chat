import { SimplifiedOperation } from '../p2panda/simplified-types';
import { Hash } from '../p2panda/types';
import { ChatId, Payload } from '../types';

/**
 * A direct chat both sides requested at once is spread over two spaces until
 * the side whose space lost the tie has published its messages again in the
 * other one, dated as first written. Until the losing space is given up, each
 * of those messages exists twice.
 */

/** The copy standing in for each message that was moved into `space`, by the
 * original's hash: a message of the same author with the same date there. */
export function movedMessages(
	ops: SimplifiedOperation<Payload>[],
	space: ChatId,
): Record<Hash, SimplifiedOperation<Payload>> {
	const messages = ops
		.filter(isMessage)
		.sort((a, b) => a.header.seq_num - b.header.seq_num);
	const candidates: Record<string, SimplifiedOperation<Payload>[]> = {};
	for (const op of messages) {
		if (op.header.topic_id !== space) continue;
		(candidates[writtenBy(op)] ??= []).push(op);
	}

	const copies: Record<Hash, SimplifiedOperation<Payload>> = {};
	for (const op of messages) {
		if (op.header.topic_id === space) continue;
		const copy = candidates[writtenBy(op)]?.shift();
		if (copy !== undefined) copies[op.hash] = copy;
	}
	return copies;
}

/** Shows each message moved into `space` once, as its copy, with whatever
 * referred to the original referring to the copy. */
export function foldMovedMessages(
	ops: SimplifiedOperation<Payload>[],
	space: ChatId,
): SimplifiedOperation<Payload>[] {
	const copies = movedMessages(ops, space);
	if (Object.keys(copies).length === 0) return ops;
	return ops
		.filter(op => copies[op.hash] === undefined)
		.map(op => retarget(op, copies));
}

function isMessage(op: SimplifiedOperation<Payload>): boolean {
	return op.body?.type === 'Chat' && op.body.payload.type === 'Message';
}

function writtenBy(op: SimplifiedOperation<Payload>): string {
	return `${op.header.verifying_key}:${op.header.timestamp}`;
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
