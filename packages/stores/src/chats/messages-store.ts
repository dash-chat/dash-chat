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

		const { messages, reactionsByTarget } = collectMessageActionsByType(logs);

		for (const [target, byAuthor] of Object.entries(reactionsByTarget)) {
			const message = messages[target];
			if (message) {
				message.reactions = byAuthor;
			} else {
				console.warn('reaction for missing message');
			}
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
}

function collectMessageActionsByType(
	logs: Record<DeviceId, SimplifiedOperation<Payload>[]>,
): {
	messages: Record<Hash, Message>;
	reactionsByTarget: Record<Hash, Record<DeviceId, string>>;
} {
	const messages: Record<Hash, Message> = {};
	const reactions: Record<Hash, Record<DeviceId, string>> = {};

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
			}
		}
	}

	return { messages, reactionsByTarget: reactions };
}
