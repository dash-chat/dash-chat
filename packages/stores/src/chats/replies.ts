import type { SimplifiedOperation } from '../p2panda/simplified-types';
import type { DeviceId, Hash } from '../p2panda/types';
import type { MediaAttachment, Payload } from '../types';
import { mediaBundleToAttachment } from '../types';
import type { Message } from './messages-store';

/** A reply annotation resolved for rendering. The quoted content is frozen at
 * the version that was replied to — later edits of the target never change
 * it — while `scrollTarget` points at the edit-chain root, which is where the
 * (possibly edited) target message is rendered. */
export type MessageReply =
	| {
			kind: 'content';
			author: DeviceId;
			text: string;
			media: MediaAttachment | null;
			/** Hash of the rendered message to scroll to (the edit-chain root). */
			scrollTarget?: Hash;
	  }
	/** The target was deleted for everyone (or is unknown locally). The quote
	 * shows only a tombstone; `scrollTarget` is set when the "deleted"
	 * placeholder message is rendered and can be scrolled to. `author` is unset
	 * when the target op itself never reached this peer. */
	| { kind: 'deleted'; author?: DeviceId; scrollTarget?: Hash }
	/** The target was deleted only on this device group — always by us, so the
	 * quote needs no author. It shows the tombstone with a warning marker and
	 * does not scroll. */
	| { kind: 'deleted-for-me' };

/** The author of the message a reply quotes, when it is known: a quote of a
 * message this peer never received has none, and a delete-for-me quote names
 * no one but us. */
export function replyAuthor(
	reply: MessageReply | undefined,
): DeviceId | undefined {
	if (reply === undefined || reply.kind === 'deleted-for-me') return undefined;
	return reply.author;
}

interface OpInfo {
	author: DeviceId;
	timestamp: number;
	body: Payload | undefined;
}

/** Every operation in the chat's logs by hash, the lookup reply resolution
 * walks: quotes and scroll targets point at ops that may not render as
 * messages themselves (edits, tombstoned bodies). */
export function collectOps(
	logs: Record<DeviceId, SimplifiedOperation<Payload>[]>,
): Record<Hash, OpInfo> {
	const ops: Record<Hash, OpInfo> = {};
	for (const operations of Object.values(logs)) {
		for (const op of operations) {
			ops[op.hash] = {
				author: op.header.verifying_key,
				timestamp: op.header.timestamp,
				body: op.body,
			};
		}
	}
	return ops;
}

function chatPayload(op: OpInfo | undefined) {
	return op?.body?.type === 'Chat' ? op.body.payload : undefined;
}

/** Walk the edit chain back from `start` to the original message and return
 * its hash, or undefined if the chain is broken or cyclic. */
function rootMessageHash(
	ops: Record<Hash, OpInfo>,
	start: Hash,
): Hash | undefined {
	let current = start;
	const limit = Object.keys(ops).length + 1;
	for (let i = 0; i < limit; i++) {
		const payload = chatPayload(ops[current]);
		if (payload?.type === 'Message') return current;
		if (payload?.type === 'EditMessage') {
			current = payload.payload.edit_hash;
			continue;
		}
		return undefined;
	}
	return undefined;
}

/** Where a delete-for-everyone placeholder for `target` is rendered: the
 * earliest op of the covering delete's chain, mirroring `deletedMessages`.
 * Undefined when no delete covers `target`. */
function deletePlaceholderHash(
	ops: Record<Hash, OpInfo>,
	target: Hash,
): Hash | undefined {
	for (const op of Object.values(ops)) {
		const payload = chatPayload(op);
		if (payload?.type !== 'DeleteMessage') continue;
		const hashes = payload.payload.hashes;
		if (!hashes.includes(target)) continue;
		return earliestCoveredHash(ops, hashes);
	}
	return undefined;
}

/** The earliest of `hashes` this peer has, by the same ordering the message
 * list uses — timestamp, then hash to break ties. Only ops that render as
 * messages are candidates: an edit that still has its body lives in the
 * edit chain, not the message map. */
function earliestCoveredHash(
	ops: Record<Hash, OpInfo>,
	hashes: Hash[],
): Hash | undefined {
	let earliest: Hash | undefined;
	for (const hash of hashes) {
		const op = ops[hash];
		if (op === undefined) continue;
		const payload = chatPayload(op);
		if (op.body !== undefined && payload?.type !== 'Message') continue;
		if (
			earliest === undefined ||
			op.timestamp < ops[earliest].timestamp ||
			(op.timestamp === ops[earliest].timestamp && hash < earliest)
		) {
			earliest = hash;
		}
	}
	return earliest;
}

/** Resolve one reply annotation against the logs, mirroring the backend's
 * receiving-side rules: a reply to anything other than a `Message` or
 * `EditMessage`, or not later than its target, is invalid (`undefined` —
 * the annotation is ignored and the message renders as a plain message). A
 * reply to a deleted or locally-unknown target is valid but shows only a
 * tombstone: deletes must completely remove content in all circumstances. */
function resolveReply(
	ops: Record<Hash, OpInfo>,
	deletedForMeHashes: Set<Hash>,
	replyTimestamp: number,
	target: Hash,
): MessageReply | undefined {
	if (deletedForMeHashes.has(target)) return { kind: 'deleted-for-me' };

	const targetOp = ops[target];
	const payload = chatPayload(targetOp);

	if (targetOp === undefined || targetOp.body === undefined) {
		return {
			kind: 'deleted',
			author: targetOp?.author,
			scrollTarget: deletePlaceholderHash(ops, target),
		};
	}
	if (payload?.type !== 'Message' && payload?.type !== 'EditMessage') {
		return undefined;
	}
	if (replyTimestamp <= targetOp.timestamp) return undefined;

	const root = rootMessageHash(ops, target);
	if (root !== undefined && deletedForMeHashes.has(root)) {
		return { kind: 'deleted-for-me' };
	}

	const rootPayload = root === undefined ? undefined : chatPayload(ops[root]);
	const media =
		rootPayload?.type === 'Message'
			? mediaBundleToAttachment(rootPayload.payload.media)
			: null;
	return {
		kind: 'content',
		author: targetOp.author,
		text: payload.payload.message,
		media,
		scrollTarget: root,
	};
}

/** The reply as it can actually be rendered against `messages`: a scroll
 * target is kept only while the message it points at is in the map, and a
 * quote whose target ended up deleted for everyone falls back to a tombstone. */
function renderableReply(
	reply: MessageReply,
	messages: Record<Hash, Message>,
): MessageReply {
	if (reply.kind === 'deleted-for-me') return reply;

	const target = reply.scrollTarget;
	const rendered = target === undefined ? undefined : messages[target];
	if (rendered === undefined) return { ...reply, scrollTarget: undefined };
	if (reply.kind === 'content' && rendered.content === 'deleted-for-everyone') {
		return { kind: 'deleted', author: reply.author, scrollTarget: target };
	}
	return reply;
}

/** Resolve the message's reply annotation and return the resulting message.
 * Must run after edits, deletes and delete-for-me tombstones have been applied
 * to `messages`: a quote pointing at a message that ended up deleted falls
 * back to a tombstone, and its scroll target is only kept when the
 * corresponding placeholder actually renders. */
export function applyReply(
	message: Message,
	messages: Record<Hash, Message>,
	ops: Record<Hash, OpInfo>,
	deletedForMeHashes: Set<Hash>,
): Message {
	if (message.replyTo === undefined) return message;

	const reply = resolveReply(
		ops,
		deletedForMeHashes,
		message.timestamp,
		message.replyTo,
	);
	if (reply === undefined) return message;

	return { ...message, reply: renderableReply(reply, messages) };
}
