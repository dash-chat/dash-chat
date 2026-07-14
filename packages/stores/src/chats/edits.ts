import type { SimplifiedOperation } from '../p2panda/simplified-types';
import type { DeviceId, Hash } from '../p2panda/types';
import type { Payload } from '../types';
import type { Message } from './messages-store';

/** The window during which a message may be edited, measured from the original
 * message timestamp. Frontend operation timestamps are milliseconds since the
 * UNIX epoch (the backend serializes them as such), so this is 24h in ms.
 * Mirrors `EDIT_WINDOW_MICROS` in `crates/dashchat-node/src/chat/edit.rs`. */
export const EDIT_WINDOW_MS = 24 * 60 * 60 * 1000;

/** A single version of a message's text, with the time it was authored. */
export interface MessageVersion {
	text: string;
	timestamp: number;
}

type ChatOpKind =
	| { kind: 'message' }
	| { kind: 'edit'; target: Hash; text: string }
	| { kind: 'other' };

interface ChatOp {
	author: DeviceId;
	timestamp: number;
	kind: ChatOpKind;
}

function collectChatOps(
	logs: Record<DeviceId, SimplifiedOperation<Payload>[]>,
): Record<Hash, ChatOp> {
	const ops: Record<Hash, ChatOp> = {};
	for (const operations of Object.values(logs)) {
		for (const op of operations) {
			const body = op.body;
			if (body?.type !== 'Chat') continue;
			let kind: ChatOpKind;
			if (body.payload.type === 'Message') {
				kind = { kind: 'message' };
			} else if (body.payload.type === 'EditMessage') {
				kind = {
					kind: 'edit',
					target: body.payload.payload.edit_hash,
					text: body.payload.payload.message,
				};
			} else {
				kind = { kind: 'other' };
			}
			ops[op.hash] = {
				author: op.header.verifying_key,
				timestamp: op.header.timestamp,
				kind,
			};
		}
	}
	return ops;
}

/** Walk the edit chain back from `start` to the original message and return its
 * hash, or undefined if the chain is broken (missing link / non-editable op) or
 * cyclic. Mirrors `root_message_timestamp` in the backend. */
function rootMessageHash(
	ops: Record<Hash, ChatOp>,
	start: Hash,
): Hash | undefined {
	let current = start;
	const limit = Object.keys(ops).length + 1;
	for (let i = 0; i < limit; i++) {
		const op = ops[current];
		if (!op) return undefined;
		if (op.kind.kind === 'message') return current;
		if (op.kind.kind === 'edit') {
			current = op.kind.target;
			continue;
		}
		return undefined;
	}
	return undefined;
}

/** Whether an edit op may be applied to its target. Mirrors `validate_edit`. */
function isValidEdit(
	ops: Record<Hash, ChatOp>,
	editHash: Hash,
	edit: ChatOp,
): boolean {
	if (edit.kind.kind !== 'edit') return false;
	const targetHash = edit.kind.target;
	const target = ops[targetHash];
	if (!target) return false;
	if (target.kind.kind === 'other') return false;
	if (edit.author !== target.author) return false;

	for (const [hash, op] of Object.entries(ops)) {
		if (hash === editHash) continue;
		if (op.kind.kind === 'edit' && op.kind.target === targetHash) return false;
	}

	const root = rootMessageHash(ops, targetHash);
	if (root === undefined) return false;
	const rootTimestamp = ops[root].timestamp;
	if (edit.timestamp - rootTimestamp > EDIT_WINDOW_MS) return false;

	return true;
}

/** Apply all valid edits in `logs` to the already-built `messages` map, in
 * place. Each edited message gets its `content.message` replaced with the
 * latest version, an `editedAt` timestamp, and a `history` of every version
 * (original first). Invalid edits are ignored, matching the backend. */
export function applyEdits(
	messages: Record<Hash, Message>,
	logs: Record<DeviceId, SimplifiedOperation<Payload>[]>,
): void {
	const ops = collectChatOps(logs);

	const editsByRoot: Record<Hash, (MessageVersion & { hash: Hash })[]> = {};
	for (const [hash, op] of Object.entries(ops)) {
		if (op.kind.kind !== 'edit') continue;
		if (!isValidEdit(ops, hash, op)) continue;
		const root = rootMessageHash(ops, hash);
		if (root === undefined) continue;
		(editsByRoot[root] ??= []).push({
			hash,
			text: op.kind.text,
			timestamp: op.timestamp,
		});
	}

	for (const [root, versions] of Object.entries(editsByRoot)) {
		const message = messages[root];
		if (!message) continue;
		versions.sort((a, b) => a.timestamp - b.timestamp);
		const latest = versions[versions.length - 1];
		message.history = [
			{ text: message.content.message, timestamp: message.timestamp },
			...versions.map(({ text, timestamp }) => ({ text, timestamp })),
		];
		message.content.message = latest.text;
		message.editedAt = latest.timestamp;
		message.latestEditHash = latest.hash;
	}
}
