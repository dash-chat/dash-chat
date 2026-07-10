import type { Message } from '../direct-chats/direct-chat-store';
import type { SimplifiedOperation } from '../p2panda/simplified-types';
import type { DeviceId, Hash } from '../p2panda/types';
import type { MediaAttachment, Payload } from '../types';
import { mediaBundleToAttachment } from '../types';

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
	 * placeholder message is rendered and can be scrolled to. */
	| { kind: 'deleted'; scrollTarget?: Hash }
	/** The target was deleted only on this device group. The quote shows the
	 * tombstone with a warning marker and does not scroll. */
	| { kind: 'deleted-for-me' };

interface OpInfo {
	author: DeviceId;
	timestamp: number;
	seqNum: number;
	body: Payload | undefined;
}

function collectOps(
	logs: Record<DeviceId, SimplifiedOperation<Payload>[]>,
): Record<Hash, OpInfo> {
	const ops: Record<Hash, OpInfo> = {};
	for (const operations of Object.values(logs)) {
		for (const op of operations) {
			ops[op.hash] = {
				author: op.header.verifying_key,
				timestamp: op.header.timestamp,
				seqNum: op.header.seq_num,
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
 * root (lowest seq num) of the covering delete's chain, mirroring
 * `applyDeletes`. Undefined when no delete covers `target`. */
function deletePlaceholderHash(
	ops: Record<Hash, OpInfo>,
	target: Hash,
): Hash | undefined {
	for (const op of Object.values(ops)) {
		const payload = chatPayload(op);
		if (payload?.type !== 'DeleteMessage') continue;
		const hashes = payload.payload.hashes;
		if (!hashes.includes(target)) continue;
		let root: Hash | undefined;
		for (const hash of hashes) {
			const member = ops[hash];
			if (!member) continue;
			if (root === undefined || member.seqNum < ops[root].seqNum) root = hash;
		}
		return root;
	}
	return undefined;
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
		return { kind: 'deleted', scrollTarget: deletePlaceholderHash(ops, target) };
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

/** Resolve the reply annotation of every message in the already-built
 * `messages` map, in place. Must run after `applyEdits`, `applyDeletes` and
 * `applyDeletesForMe`: a quote pointing at a message that ended up deleted
 * falls back to a tombstone, and its scroll target is only kept when the
 * corresponding placeholder actually renders. */
export function applyReplies(
	messages: Record<Hash, Message>,
	logs: Record<DeviceId, SimplifiedOperation<Payload>[]>,
	deletedForMeHashes: Set<Hash>,
): void {
	const ops = collectOps(logs);

	for (const message of Object.values(messages)) {
		if (message.replyTo === undefined) continue;
		const reply = resolveReply(
			ops,
			deletedForMeHashes,
			message.timestamp,
			message.replyTo,
		);
		if (reply === undefined) continue;

		if (reply.kind === 'content' || reply.kind === 'deleted') {
			const target = reply.scrollTarget;
			const rendered = target === undefined ? undefined : messages[target];
			if (rendered === undefined) {
				reply.scrollTarget = undefined;
			} else if (reply.kind === 'content' && rendered.deleted === true) {
				message.reply = { kind: 'deleted', scrollTarget: target };
				continue;
			}
		}
		message.reply = reply;
	}
}
