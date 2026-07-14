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
 * UNIX epoch (the backend serializes them as such), so this is 24h in ms.
 * Mirrors `EDIT_WINDOW_MICROS` in `crates/dashchat-node/src/chat/edit.rs`. */
export const EDIT_WINDOW_MS = 24 * 60 * 60 * 1000;

/** A single version of a message's text, with the time it was authored. */
export interface MessageVersion {
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
	/** Timestamp of the latest edit, if the message has been edited. */
	editedAt?: number;
	/** Every version of the text, original first, when the message was edited. */
	history?: MessageVersion[];
	/** Hash of the latest edit op in the chain; the target for the next edit. */
	latestEditHash?: Hash;
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
		const target = message.latestEditHash ?? message.hash;
		return this.client.editMessage(chatId, target, newText);
	}
}

/** An edit op: the new text, keyed in `editsByTarget` by the hash it edits. */
interface Edit {
	hash: Hash;
	text: string;
	timestamp: number;
}

function collectMessageActionsByType(
	logs: Record<DeviceId, SimplifiedOperation<Payload>[]>,
): {
	messages: Record<Hash, Message>;
	reactionsByTarget: Record<Hash, Record<DeviceId, string>>;
	editsByTarget: Record<Hash, Edit>;
} {
	const messages: Record<Hash, Message> = {};
	const reactions: Record<Hash, Record<DeviceId, string>> = {};
	const edits: Record<Hash, Edit> = {};

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
				};
			} else if (body.payload.type === 'Reaction') {
				const { target, emoji } = body.payload.payload;
				if (reactions[target] === undefined) {
					reactions[target] = {};
				}
				if (emoji) {
					reactions[target][author] = emoji;
				} else {
					delete reactions[target][author];
				}
			} else if (body.payload.type === 'EditMessage') {
				edits[body.payload.payload.edit_hash] = {
					hash: operation.hash,
					text: body.payload.payload.message,
					timestamp: operation.header.timestamp,
				};
			}
		}
	}

	return { messages, reactionsByTarget: reactions, editsByTarget: edits };
}

// Apply the message's edit chain and return the resulting message
function applyEdits(
	message: Message,
	editsByTarget: Record<Hash, Edit>,
): Message {
	const versions: Edit[] = [];
	const seen = new Set<Hash>([message.hash]);
	for (
		let edit = editsByTarget[message.hash];
		edit !== undefined && !seen.has(edit.hash);
		edit = editsByTarget[edit.hash]
	) {
		seen.add(edit.hash);
		versions.push(edit);
	}
	if (versions.length === 0) return message;

	const latest = versions[versions.length - 1];
	return {
		...message,
		content: { ...message.content, message: latest.text },
		history: [
			{ text: message.content.message, timestamp: message.timestamp },
			...versions.map(({ text, timestamp }) => ({ text, timestamp })),
		],
		editedAt: latest.timestamp,
		latestEditHash: latest.hash,
	};
}
