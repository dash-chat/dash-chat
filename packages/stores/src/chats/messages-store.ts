import { type ReactiveFn, reactive } from 'signalium';

import { ContactsStore } from '../contacts/contacts-store';
import { LogsStore } from '../p2panda/logs-store';
import { SimplifiedOperation } from '../p2panda/simplified-types';
import { DeviceId, Hash } from '../p2panda/types';
import {
	ChatId,
	ChatReaction,
	MediaAttachment,
	OutgoingMedia,
	Payload,
	mediaBundleToAttachment,
} from '../types';
import { type IMessagesClient } from './messages-client';

/** The window during which a message may be edited, measured from the original
 * message timestamp. Frontend operation timestamps are milliseconds since the
 * UNIX epoch (the backend serializes them as such), so this is 24h in ms. */
export const EDIT_WINDOW_MS = 24 * 60 * 60 * 1000;

export interface MessageVersion {
	hash: string;
	text: string;
	timestamp: number;
}

export interface Message {
	hash: string;
	content: {
		message: string;
		media: MediaAttachment | null;
	};
	timestamp: number;
	author: DeviceId;
	seqNum: number;
	reactions: Record<DeviceId, string>;
	editHistory: MessageVersion[];
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

		const { messages, reactionsByTarget, editsByTarget } =
			collectMessageActionsByType(logs);

		for (const [target, byAuthor] of Object.entries(reactionsByTarget)) {
			const message = messages[target];
			if (message) {
				message.reactions = byAuthor;
			} else {
				console.warn('reaction for missing message');
			}
		}

		for (const [hash, message] of Object.entries(messages)) {
			messages[hash] = applyEdits(message, editsByTarget);
		}

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

		// Callers hold a snapshot captured when editing began; re-resolve so an
		// edit that arrived mid-compose is chained from, not forked off.
		const fresh = (await this.messages())[message.hash] ?? message;
		const current = currentVersion(fresh);
		return this.client.editMessage(chatId, current.hash, newText);
	}
}

function collectMessageActionsByType(
	logs: Record<DeviceId, SimplifiedOperation<Payload>[]>,
): {
	messages: Record<Hash, Message>;
	reactionsByTarget: Record<Hash, Record<DeviceId, string>>;
	editsByTarget: Record<Hash, Record<Hash, MessageVersion>>;
} {
	const messages: Record<Hash, Message> = {};
	const reactionsByTarget: Record<Hash, Record<DeviceId, string>> = {};
	const editsByTarget: Record<Hash, Record<Hash, MessageVersion>> = {};

	for (const [author, operations] of Object.entries(logs)) {
		for (const operation of operations) {
			const body = operation.body;
			if (body?.type !== 'Chat') continue;
			if (body.payload.type === 'Message') {
				messages[operation.hash] = {
					hash: operation.hash,
					content: {
						message: body.payload.payload.message,
						media: mediaBundleToAttachment(body.payload.payload.media),
					},
					author,
					seqNum: operation.header.seq_num,
					timestamp: operation.header.timestamp,
					reactions: {},
					editHistory: [],
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
			}
		}
	}

	return {
		messages,
		reactionsByTarget,
		editsByTarget,
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
		content: { ...message.content, message: latest.text },
		editHistory: versions,
	};
}

function currentVersion(message: Message): MessageVersion {
	if (message.editHistory.length > 0) {
		return message.editHistory[message.editHistory.length - 1];
	}
	return {
		hash: message.hash,
		text: message.content.message,
		timestamp: message.timestamp,
	};
}
