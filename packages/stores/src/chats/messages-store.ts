import { type ReactiveFn, reactive } from 'signalium';

import { ContactsStore } from '../contacts/contacts-store';
import { LogsStore } from '../p2panda/logs-store';
import { SimplifiedOperation } from '../p2panda/simplified-types';
import { DeviceId, Hash } from '../p2panda/types';
import { TombstoneStore } from '../tombstones/tombstone-store';
import {
	ChatId,
	ChatReaction,
	MessageDisplay,
	MessageVersion,
	OutgoingMedia,
	Payload,
	Tombstone,
	hasBody,
	mediaBundleToAttachment,
} from '../types';
import { type IMessagesClient } from './messages-client';
import { type MessageReply, applyReplies } from './replies';

/** The window during which a message may be edited, measured from the original
 * message timestamp. Frontend operation timestamps are milliseconds since the
 * UNIX epoch (the backend serializes them as such), so this is 24h in ms. */
export const EDIT_WINDOW_MS = 24 * 60 * 60 * 1000;

/** The window during which a message may be deleted for everyone, measured
 * from the original message timestamp. Deleting a message for yourself is
 * always allowed. Mirrors `DELETE_WINDOW_MICROS` in
 * `crates/dashchat-node/src/chat/validation/delete.rs` (frontend timestamps
 * are ms). */
export const DELETE_FOR_EVERYONE_WINDOW_MS = 24 * 60 * 60 * 1000;

export interface Message {
	hash: string;
	/** The message payload with its reactions and edit history, or
	 * `'deleted-for-everyone'` once deleted. */
	content: MessageDisplay;
	timestamp: number;
	author: DeviceId;
	seqNum: number;
	/** Raw hash of the operation this message replies to, straight from the log. */
	replyTo?: Hash;
	/** The reply resolved for rendering; unset when the annotation is invalid. */
	reply?: MessageReply;
}

// The messages of a single chat, direct or group alike: the message log with
// reactions and read-tracking, plus the actions to publish into it.
export class MessagesStore {
	constructor(
		protected logsStore: LogsStore<Payload>,
		protected contactsStore: ContactsStore,
		protected tombstoneStore: TombstoneStore,
		/** Resolves to '' while a pending direct chat has no topic yet. */
		public chatId: ReactiveFn<Promise<ChatId>, []>,
		public client: IMessagesClient,
	) {}

	messages = reactive(async () => {
		const chatId = await this.chatId();
		if (chatId === '') return {} as Record<Hash, Message>;
		const logs = await this.logsStore.logsForAllAuthors(chatId);

		const {
			messages,
			bodylessOps,
			reactionsByTarget,
			editsByTarget,
			deleteTargets,
		} = collectMessageActionsByType(logs);

		for (const [target, byAuthor] of Object.entries(reactionsByTarget)) {
			const message = messages[target];
			if (message === undefined) {
				// A reaction on a tombstoned op is moot, not a missing target.
				if (bodylessOps[target] === undefined) {
					console.warn('reaction for missing message');
				}
			} else if (hasBody(message.content)) {
				message.content.reactions = byAuthor;
			}
		}

		for (const [hash, message] of Object.entries(messages)) {
			messages[hash] = applyEdits(message, editsByTarget);
		}

		Object.assign(
			messages,
			deletedMessages(deleteTargets, messages, bodylessOps),
		);

		// Completely remove any message with a DeletedForMe tombstone.
		const tombstones = await this.tombstoneStore.tombstones(chatId);
		for (const { hash, reason } of tombstones) {
			if (reason === 'DeletedForMe') delete messages[hash];
		}

		applyReplies(messages, logs, deletedForMeHashes(tombstones));

		return messages;
	});

	lastMessage = reactive(async () => {
		const messages = await this.messages();

		const sortedMessages = Object.values(messages).sort(
			(m1, m2) => m2.timestamp - m1.timestamp,
		);
		return sortedMessages.length > 0 ? sortedMessages[0] : undefined;
	});

	readMessageHashes = reactive(async () => {
		const chatId = await this.chatId();
		const myDeviceGroupTopic =
			await this.contactsStore.devicesStore.myDeviceGroupTopic();
		const readHashes: Set<Hash> = new Set();

		for (const [_, ops] of Object.entries(myDeviceGroupTopic)) {
			for (const op of ops) {
				if (
					op.body?.payload?.type === 'ReadMessages' &&
					op.body.payload.payload.chat_id === chatId
				) {
					for (const hash of op.body.payload.payload.message_hashes) {
						readHashes.add(hash);
					}
				}
			}
		}

		return readHashes;
	});

	unreadCount = reactive(async () => {
		const messages = await this.messages();
		const readHashes = await this.readMessageHashes();
		const myDeviceId = await this.contactsStore.myDeviceId();

		let count = 0;
		for (const [hash, message] of Object.entries(messages)) {
			// Only count messages from others (not our own)
			if (message.author !== myDeviceId && !readHashes.has(hash)) {
				count++;
			}
		}
		return count;
	});

	/** Sends the message and resolves with the operation id of the created
	 * message once it is confirmed in the local log. When `replyTo` is set,
	 * the message is sent as a reply to that message's latest known edit. */
	async sendMessage(input: {
		message: string;
		media: OutgoingMedia | null;
		replyTo?: Message | null;
	}): Promise<Hash> {
		const chatId = await this.chatId();

		// A reply targets the latest known edit of the message, like edits and
		// deletes do. Same staleness concern as `editMessage`: re-resolve from
		// the current map rather than the caller's snapshot.
		let reply: Hash | null = null;
		if (input.replyTo) {
			const fresh =
				(await this.messages())[input.replyTo.hash] ?? input.replyTo;
			reply = currentVersion(fresh).hash;
		}
		return this.client.sendMessage(chatId, input.message, input.media, reply);
	}

	async markAsRead(messageHashes: Hash[]): Promise<void> {
		const chatId = await this.chatId();
		await this.client.markMessagesRead(chatId, messageHashes);
	}

	async sendReaction(reaction: ChatReaction) {
		const chatId = await this.chatId();
		await this.client.sendReaction(chatId, reaction);
	}

	async editMessage(message: Message, newText: string): Promise<Hash> {
		const chatId = await this.chatId();

		// Callers hold a snapshot captured when editing began; re-resolve so an
		// edit that arrived mid-compose is chained from, not forked off.
		const fresh = (await this.messages())[message.hash] ?? message;
		const current = currentVersion(fresh);
		return this.client.editMessage(chatId, current.hash, newText);
	}

	async deleteMessageForEveryone(message: Message): Promise<Hash> {
		const chatId = await this.chatId();
		// Same staleness concern as `editMessage`: the caller's snapshot may
		// predate an edit that arrived since.
		const fresh = (await this.messages())[message.hash] ?? message;
		const current = currentVersion(fresh);
		return this.client.deleteMessageForEveryone(chatId, current.hash);
	}

	async deleteMessageForMe(message: Message): Promise<Hash> {
		const chatId = await this.chatId();
		// The whole message is deleted regardless of which version is shown, so
		// target the original op; the backend tombstones its edit chain.
		return this.client.deleteMessageForMe(chatId, message.hash);
	}
}

function deletedForMeHashes(tombstones: Tombstone[]): Set<Hash> {
	return new Set(
		tombstones.filter(t => t.reason === 'DeletedForMe').map(t => t.hash),
	);
}

function collectMessageActionsByType(
	logs: Record<DeviceId, SimplifiedOperation<Payload>[]>,
): {
	messages: Record<Hash, Message>;
	// Operations whose payload is gone, keyed by hash
	bodylessOps: Record<Hash, SimplifiedOperation<void>>;
	reactionsByTarget: Record<Hash, Record<DeviceId, string>>;
	editsByTarget: Record<Hash, Record<Hash, MessageVersion>>;
	/** One entry per delete: every hash it covers — the original message and its
	 * edits. */
	deleteTargets: Hash[][];
} {
	const messages: Record<Hash, Message> = {};
	const bodylessOps: Record<Hash, SimplifiedOperation<void>> = {};
	const reactionsByTarget: Record<Hash, Record<DeviceId, string>> = {};
	const editsByTarget: Record<Hash, Record<Hash, MessageVersion>> = {};
	const deleteTargets: Hash[][] = [];

	for (const [author, operations] of Object.entries(logs)) {
		for (const operation of operations) {
			const body = operation.body;
			if (!body) {
				if (operation.header.auth) continue;
				bodylessOps[operation.hash] = { ...operation, body: undefined };
				continue;
			}
			if (body.type !== 'Chat') continue;
			if (body.payload.type === 'Message') {
				messages[operation.hash] = {
					hash: operation.hash,
					content: {
						message: body.payload.payload.message,
						media: mediaBundleToAttachment(body.payload.payload.media),
						reactions: {},
						editHistory: [],
					},
					author,
					seqNum: operation.header.seq_num,
					timestamp: operation.header.timestamp,
					replyTo: body.payload.payload.reply,
				};
			} else if (body.payload.type === 'Reaction') {
				const { target, emoji } = body.payload.payload;
				if (reactionsByTarget[target] === undefined) {
					reactionsByTarget[target] = {};
				}
				if (emoji) {
					reactionsByTarget[target][author] = emoji;
				} else {
					delete reactionsByTarget[target][author];
				}
			} else if (body.payload.type === 'EditMessage') {
				const target = body.payload.payload.edit_hash;
				if (editsByTarget[target] === undefined) {
					editsByTarget[target] = {};
				}
				editsByTarget[target][operation.hash] = {
					hash: operation.hash,
					text: body.payload.payload.message,
					timestamp: operation.header.timestamp,
				};
			} else if (body.payload.type === 'DeleteMessage') {
				deleteTargets.push(body.payload.payload.hashes);
			}
		}
	}

	return {
		messages,
		bodylessOps,
		reactionsByTarget,
		editsByTarget,
		deleteTargets,
	};
}

/** The placeholder each delete leaves behind, keyed by the hash it stands in
 * for. Merged over `messages` these replace the original's live entry, the only
 * covered op that can be there — edits live in `editsByTarget`. */
function deletedMessages(
	deleteTargets: Hash[][],
	messages: Record<Hash, Message>,
	bodylessOps: Record<Hash, SimplifiedOperation<void>>,
): Record<Hash, Message> {
	const deleted: Record<Hash, Message> = {};
	for (const hashes of deleteTargets) {
		const original = earliestOp(hashes, messages, bodylessOps);
		if (original === undefined) continue;
		deleted[original.hash] = { ...original, content: 'deleted-for-everyone' };
	}
	return deleted;
}

/** The earliest of `hashes` this peer has — the original message, since its
 * edits are authored after it. Ordered by timestamp, as the message list itself
 * is, so the placeholder lands in the slot the original occupied. `seq_num`
 * can't be used: once devices are linked an edit may come from another device,
 * and positions in different logs aren't comparable. The hash breaks ties so
 * every peer eventually picks the same op. */
function earliestOp(
	hashes: Hash[],
	messages: Record<Hash, Message>,
	bodylessOps: Record<Hash, SimplifiedOperation<void>>,
): Message | undefined {
	let earliest: Message | undefined;
	for (const hash of hashes) {
		const bodyless = bodylessOps[hash];
		const op =
			messages[hash] ??
			(bodyless === undefined ? undefined : placeholderFor(bodyless));
		if (op === undefined) continue;
		if (
			earliest === undefined ||
			op.timestamp < earliest.timestamp ||
			(op.timestamp === earliest.timestamp && op.hash < earliest.hash)
		) {
			earliest = op;
		}
	}
	return earliest;
}

/** The placeholder a tombstoned operation renders as, built from its header
 * since its payload is gone. */
function placeholderFor(op: SimplifiedOperation<void>): Message {
	return {
		hash: op.hash,
		content: 'deleted-for-everyone',
		author: op.header.verifying_key,
		seqNum: op.header.seq_num,
		timestamp: op.header.timestamp,
	};
}

// Apply the message's edits and return the resulting message. Every edit
// reachable from the message — following chains through `editsByTarget`,
// across forks — becomes a version; the one with the highest timestamp is the
// displayed text.
//
// TODO(after p2panda-spaces integration): this trusts every edit op in the
// raw logs and enforces none of the backend's edit-validation rules
// (`ValidChatOps::validate_edit` in crates/dashchat-node/src/chat/edit.rs):
// author-only, at most one edit per target resolved by (seq_num, hash), the
// 24h edit window, and target-must-be-editable. A misbehaving peer's ops
// would therefore render here. Once p2panda-spaces is integrated the
// frontend should consume validated logs (or mirror validate_edit) instead.
function applyEdits(
	message: Message,
	editsByTarget: Record<Hash, Record<Hash, MessageVersion>>,
): Message {
	// Deletes are applied after edits, so live content is always present here.
	if (!hasBody(message.content)) return message;
	const versions: MessageVersion[] = [];
	const seen = new Set<Hash>([message.hash]);
	const pending = Object.values(editsByTarget[message.hash] ?? {});
	while (pending.length > 0) {
		const edit = pending.pop();
		if (edit === undefined || seen.has(edit.hash)) continue;
		seen.add(edit.hash);
		versions.push(edit);
		pending.push(...Object.values(editsByTarget[edit.hash] ?? {}));
	}
	if (versions.length === 0) return message;

	versions.sort((v1, v2) => v1.timestamp - v2.timestamp);
	const latest = versions[versions.length - 1];
	return {
		...message,
		content: {
			...message.content,
			message: latest.text,
			editHistory: versions,
		},
	};
}

function currentVersion(message: Message): MessageVersion {
	if (!hasBody(message.content)) {
		return { hash: message.hash, text: '', timestamp: message.timestamp };
	}
	const { editHistory } = message.content;
	if (editHistory.length > 0) {
		return editHistory[editHistory.length - 1];
	}
	return {
		hash: message.hash,
		text: message.content.message,
		timestamp: message.timestamp,
	};
}
