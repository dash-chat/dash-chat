import { type ReactiveFn, reactive } from 'signalium';

import { ContactsStore } from '../contacts/contacts-store';
import { LogsStore } from '../p2panda/logs-store';
import { SimplifiedOperation } from '../p2panda/simplified-types';
import { DeviceId, Hash } from '../p2panda/types';
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
}

// The messages of a single chat, direct or group alike: the message log with
// reactions and read-tracking, plus the actions to publish into it.
export class MessagesStore {
	constructor(
		protected logsStore: LogsStore<Payload>,
		protected contactsStore: ContactsStore,
		/** Resolves to '' while a pending direct chat has no topic yet. */
		public chatId: ReactiveFn<Promise<ChatId>, []>,
		public client: IMessagesClient,
	) {}

	messages = reactive(async () => {
		const chatId = await this.chatId();
		if (chatId === '') return {} as Record<Hash, Message>;
		const logs = await this.logsStore.logsForAllAuthors(chatId);

		const { messages, reactionsByTarget, editsByTarget, deletes } =
			collectMessageActionsByType(logs);

		for (const [target, byAuthor] of Object.entries(reactionsByTarget)) {
			const message = messages[target];
			if (message === undefined) {
				console.warn('reaction for missing message');
				// Deletes are applied last, so live content is always present here.
			} else if (hasBody(message.content)) {
				message.content.reactions = byAuthor;
			}
		}

		for (const [hash, message] of Object.entries(messages)) {
			messages[hash] = applyEdits(message, editsByTarget);
		}

		applyDeletes(messages, deletes);

		applyTombstones(messages, await this.tombstones());

		return messages;
	});

	// The backend's tombstone set for this chat (each hash tagged with why it was
	// tombstoned). Re-fetched whenever the chat log or the device-group log
	// changes, since that's when a delete — or a transitively-tombstoned edit —
	// gets recorded. Delete-for-me can't be derived from the raw ops the way
	// reads are: the backend drops the tombstoned edits' bodies, so their
	// edit-chain pointers are gone and only the backend knows the full set.
	tombstones = reactive(async () => {
		const chatId = await this.chatId();
		if (chatId === '') return [];
		// Establish reactive dependencies on the logs so this recomputes when a
		// delete or a tombstoned edit is processed into either topic.
		await this.logsStore.logsForAllAuthors(chatId);
		await this.contactsStore.devicesStore.myDeviceGroupTopic();
		return this.client.getTombstones(chatId);
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

	async sendMessage(input: {
		message: string;
		media: OutgoingMedia | null;
	}): Promise<Hash> {
		const chatId = await this.chatId();

		return this.client.sendMessage(chatId, input.message, input.media);
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
		const current = currentVersion(message);
		return this.client.editMessage(chatId, current.hash, newText);
	}

	async deleteMessageForEveryone(message: Message): Promise<Hash> {
		const chatId = await this.chatId();
		const current = currentVersion(message);
		return this.client.deleteMessageForEveryone(chatId, current.hash);
	}

	async deleteMessageForMe(message: Message): Promise<Hash> {
		const chatId = await this.chatId();
		// The whole message is deleted regardless of which version is shown, so
		// target the original op; the backend tombstones its edit chain.
		return this.client.deleteMessageForMe(chatId, message.hash);
	}
}

/** Reconcile the built message map with the backend's tombstone set.
 *
 * A `DeletedForMe` op (and any of its edits) is removed outright — following
 * Signal, a delete-for-me leaves no placeholder. A `DeletedForEveryone` edit
 * that slipped through as a raw body-less placeholder (rather than the
 * chain-collapsed `'deleted-for-everyone'` root that `applyDeletes` produces
 * from the `DeleteMessage` op) is removed too, so only the root placeholder
 * remains. Body-less ops that are *not* tombstoned stay as `'body-unavailable'`
 * — a genuinely unfetched message, which the tombstone set lets us tell apart
 * from a deletion. */
function applyTombstones(
	messages: Record<Hash, Message>,
	tombstones: Tombstone[],
): void {
	for (const { hash, reason } of tombstones) {
		const message = messages[hash];
		if (message === undefined) continue;
		if (reason === 'DeletedForMe' || message.content === 'body-unavailable') {
			delete messages[hash];
		}
	}
}

/** A delete op, keyed in `deletesByTarget` by the original message it
 * deletes. */
interface Delete {
	hash: Hash;
	timestamp: number;
	/** The complete chain covered: the original message plus every edit. */
	hashes: Hash[];
}

function collectMessageActionsByType(
	logs: Record<DeviceId, SimplifiedOperation<Payload>[]>,
): {
	messages: Record<Hash, Message>;
	reactionsByTarget: Record<Hash, Record<DeviceId, string>>;
	editsByTarget: Record<Hash, Record<Hash, MessageVersion>>;
	/** Delete ops keyed by their own op hash; each carries the full chain of
	 * message hashes it covers. */
	deletes: Record<Hash, Delete>;
} {
	const messages: Record<Hash, Message> = {};
	const reactionsByTarget: Record<Hash, Record<DeviceId, string>> = {};
	const editsByTarget: Record<Hash, Record<Hash, MessageVersion>> = {};
	const deletes: Record<Hash, Delete> = {};

	for (const [author, operations] of Object.entries(logs)) {
		for (const operation of operations) {
			const body = operation.body;
			if (!body) {
				// The only body-less ops in a chat log are group-control ops
				// (which carry their action in `header.auth`) and messages whose
				// payload was tombstoned by a delete. Record the latter as a
				// placeholder: the DeleteMessage op that references it confirms it
				// as deleted (see `applyDeletes`); an unreferenced one is an
				// anomaly rendered as an error bubble (`'body-unavailable'`).
				if (operation.header.auth) continue;
				messages[operation.hash] = {
					hash: operation.hash,
					content: 'body-unavailable',
					author,
					seqNum: operation.header.seq_num,
					timestamp: operation.header.timestamp,
				};
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
				deletes[operation.hash] = {
					hash: operation.hash,
					timestamp: operation.header.timestamp,
					hashes: body.payload.payload.hashes,
				};
			}
		}
	}

	return {
		messages,
		reactionsByTarget,
		editsByTarget,
		deletes,
	};
}

/** Mark deleted messages as such, collapsing each delete's chain (the original
 * message plus its edits) to a single placeholder: keep the original (lowest
 * seq_num, since edits always follow it in the same author's log) as the
 * `'deleted-for-everyone'` placeholder and drop the rest. */
function applyDeletes(
	messages: Record<Hash, Message>,
	deletes: Record<Hash, Delete>,
): void {
	for (const del of Object.values(deletes)) {
		// A chain member is absent from `messages` when it's an edit op that still
		// has its body (edits live in `editsByTarget`, not `messages`) or an op
		// this peer hasn't synced yet. Tombstoned members are *not* absent: they
		// stay as body-less records. Skip the delete entirely until at least one
		// target is present.
		//
		// We trust the backend to deliver only valid operations, so this is not a check
		// for validity.
		const chain = del.hashes.filter(hash => messages[hash] !== undefined);
		if (chain.length === 0) continue;

		const original = chain.reduce((a, b) =>
			messages[a].seqNum <= messages[b].seqNum ? a : b,
		);
		for (const hash of chain) {
			if (hash === original) {
				messages[hash].content = 'deleted-for-everyone';
			} else {
				delete messages[hash];
			}
		}
	}
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
