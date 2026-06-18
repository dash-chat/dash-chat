import { reactive } from 'signalium';

import { fullName } from '../contacts/contacts-client';
import { ContactsStore } from '../contacts/contacts-store';
import { LogsStore } from '../p2panda/logs-store';
import { SimplifiedOperation } from '../p2panda/simplified-types';
import { AgentId, DeviceId, Hash } from '../p2panda/types';
import {
	ChatReaction,
	ChatSummary,
	Media,
	MessageContent,
	MessagesStore,
	Payload,
	getMessageMedia,
} from '../types';
import { EventWithProvenance, orderInEventSets } from '../utils/event-sets';
import { type IDirectChatClient } from './direct-chat-client';

export interface Message {
	hash: string;
	content: {
		message: string;
		media: Media | null;
	};
	timestamp: number;
	author: DeviceId;
	seqNum: number;
	reactions: Record<DeviceId, string>;
}

// Store tied to a specific direct chat
export class DirectChatStore implements MessagesStore {
	constructor(
		protected logsStore: LogsStore<Payload>,
		protected contactsStore: ContactsStore,
		public client: IDirectChatClient,
		public peer: AgentId,
	) {}

	chatId = reactive(async () => await this.client.chatId(this.peer));

	peerProfile = reactive(async () => {
		const request = await this.contactRequest();
		if (request) return request.profile;
		return await this.contactsStore.profiles(this.peer);
	});

	contactRequest = reactive(async () => {
		const contactRequests = await this.contactsStore.contactRequests();
		return contactRequests.find(cr => cr.code.agent_id === this.peer);
	});

	messages = reactive(async () => {
		const chatId = await this.chatId();
		const logs = await this.logsStore.logsForAllAuthors(chatId);

		const messages: Record<Hash, Message> = {};

		for (const [author, operations] of Object.entries(logs)) {
			for (const operation of operations) {
				const body = operation.body;
				if (body?.type === 'Chat') {
					if (body.payload.type === 'Message') {
						messages[operation.hash] = {
							hash: operation.hash,
							content: {
								message: body.payload.payload.message,
								media: getMessageMedia(body.payload.payload),
							},
							author,
							seqNum: operation.header.seq_num,
							timestamp: operation.header.timestamp,
							reactions: {},
						};
					}
				}
			}
		}
		// reactions applied in second loop after messages are fully resolved
		for (const [author, operations] of Object.entries(logs)) {
			for (const operation of operations) {
				const body = operation.body;
				if (body?.type === 'Chat') {
					if (body.payload.type === 'Reaction') {
						const payload = body.payload.payload;
						let message = messages[payload.target];
						if (message) {
							if (payload.emoji) {
								message.reactions[author] = payload.emoji;
							} else {
								delete message.reactions[author];
							}
						} else {
							console.warn('reaction for missing message');
						}
					}
				}
			}
		}
		return messages;
	});

	messageSets = reactive(async () => {
		const messages = await this.messages();

		const eventsWithProvenance: Record<Hash, EventWithProvenance<Message>> = {};
		const devices = new Set<DeviceId>();

		for (const [hash, message] of Object.entries(messages)) {
			devices.add(message.author);
			eventsWithProvenance[hash] = {
				event: message,
				author: message.author,
				timestamp: message.timestamp,
				type: 'Message',
			};
		}

		const agentsSets = Array.from(devices).map(a => [a]);

		const messagesWithProvenance = orderInEventSets(
			eventsWithProvenance,
			agentsSets,
		);
		return messagesWithProvenance;
	});

	lastMessage = reactive(async () => {
		const messages = await this.messages();

		const sortedMessages = Object.values(messages).sort(
			(m1, m2) => m2.timestamp - m1.timestamp,
		);
		return sortedMessages.length > 0 ? sortedMessages[0] : undefined;
	});

	onNewMessage(
		handler: (operation: SimplifiedOperation<Payload>, message: string) => void,
	) {
		return this.logsStore.logsClient.onNewOperation(async (topicId, op) => {
			const chatId = await this.chatId();
			if (topicId !== chatId) return;
			if (op.body?.payload.type !== 'Message') return;
			handler(op, op.body.payload.payload.message);
		});
	}

	async sendMessage(input: {
		message: string;
		media: Media | null;
	}): Promise<Hash> {
		const chatId = await this.chatId();
		const content: MessageContent = {
			v: '1',
			message: input.message,
			media: input.media,
		};
		const hash = await this.client.sendMessage(chatId, content);

		return hash;
	}

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

	summary = reactive(async (): Promise<ChatSummary> => {
		const profile = await this.peerProfile();
		const message = await this.lastMessage();
		const unreadCount = await this.unreadCount();

		const lastEvent: ChatSummary['lastEvent'] = message
			? {
					kind: 'message',
					content: message.content,
					timestamp: message.timestamp,
				}
			: {
					kind: 'contact_added',
					timestamp:
						(await this.contactsStore.contactAddedTimestamp(this.peer)) ?? 0,
				};

		return {
			type: 'DirectChat',
			chatId: this.peer,
			name: profile ? fullName(profile) : '',
			avatar: profile?.avatar,
			lastEvent,
			unreadMessages: unreadCount,
		};
	});

	async markAsRead(messageHashes: Hash[]): Promise<void> {
		const chatId = await this.chatId();
		await this.client.markMessagesRead(chatId, messageHashes);
	}

	async sendReaction(reaction: ChatReaction) {
		const chatId = await this.chatId();
		await this.client.sendReaction(chatId, reaction);
	}
}
