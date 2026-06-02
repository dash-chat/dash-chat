import { reactive } from 'signalium';

import { Profile } from '../contacts/contacts-client';
import { ContactsStore } from '../contacts/contacts-store';
import { Message } from '../direct-chats/direct-chat-store';
import { waitForOperation } from '../p2panda/logs-client';
import { LogsStore } from '../p2panda/logs-store';
import { AgentId, DeviceId, Hash, PublicKey } from '../p2panda/types';
import {
	ChatId,
	ChatSummary,
	MessageContent,
	Payload,
	getMessageText,
} from '../types';
import { EventWithProvenance, orderInEventSets } from '../utils/event-sets';
import { type IGroupChatClient } from './group-chat-client';

export interface GroupInfo {
	name: string | undefined;
	description: string | undefined;
	avatar: string | undefined;
}

export interface GroupMember {
	agentId: AgentId;
	profile: Profile | undefined;
	admin: boolean;
}

export class GroupChatStore {
	constructor(
		protected logsStore: LogsStore<Payload>,
		protected contactsStore: ContactsStore,
		public client: IGroupChatClient,
		public chatId: ChatId,
	) {}

	info = reactive(async () => {
		const info: GroupInfo = {
			name: 'mygroup',
			description: undefined,
			avatar: undefined,
		};
		return info;
	});

	messages = reactive(async () => {
		const logs = await this.logsStore.logsForAllAuthors(this.chatId);

		const messages: Record<Hash, Message> = {};

		for (const [author, operations] of Object.entries(logs)) {
			for (const operation of operations) {
				const body = operation.body;
				if (body?.type === 'Chat') {
					if (body.payload.type === 'Message') {
						messages[operation.hash] = {
							hash: operation.hash,
							content: getMessageText(body.payload.payload),
							author,
							seqNum: operation.header.seq_num,
							timestamp: operation.header.timestamp,
							reactions: {},
						};
					}
				}
			}
		}
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

		return orderInEventSets(eventsWithProvenance, agentsSets);
	});

	lastMessage = reactive(async () => {
		const messages = await this.messages();

		const sortedMessages = Object.values(messages).sort(
			(m1, m2) => m2.timestamp - m1.timestamp,
		);
		return sortedMessages.length > 0 ? sortedMessages[0] : undefined;
	});

	membersData = reactive(async () => {
		return await this.client.getMembers(this.chatId);
	});

	me = reactive(async () => {
		const myAgentId = await this.contactsStore.myAgentId();
		const data = await this.membersData();
		const entry = data.find(m => m.agentId === myAgentId);
		return this.buildMember(myAgentId, entry?.isAdmin ?? false);
	});

	allMembers = reactive(async () => {
		const data = await this.membersData();
		const entries = await Promise.all(
			data.map(async ({ agentId, isAdmin }) => {
				const member = await this.buildMember(agentId, isAdmin);
				return [agentId, member] as const;
			}),
		);
		return Object.fromEntries(entries) as Record<AgentId, GroupMember>;
	});

	private buildMember = reactive(async (agentId: AgentId, admin: boolean) => {
		const profile = await this.contactsStore.profiles(agentId);
		return { agentId, profile, admin } satisfies GroupMember;
	});

	summary = reactive(async (): Promise<ChatSummary> => {
		const info = await this.info();
		const last = await this.lastMessage();

		return {
			type: 'GroupChat',
			chatId: this.chatId,
			name: info.name ?? '',
			avatar: info.avatar,
			lastEvent: {
				summary: last?.content ?? '',
				timestamp: last?.timestamp ?? 0,
			},
			unreadMessages: 0,
		};
	});

	/// Actions

	addMember(member: PublicKey) {
		return this.client.addMember(this.chatId, member);
	}

	async sendMessage(text: string) {
		const myDeviceId = await this.contactsStore.myDeviceId();
		const content: MessageContent = { v: '1', message: text, media: null };
		await Promise.all([
			waitForOperation(this.logsStore.logsClient, (op, topicId) => {
				if (topicId !== this.chatId) return false;
				if (op.body?.payload.type !== 'Message') return false;
				if (op.header.public_key !== myDeviceId) return false;
				if (getMessageText(op.body.payload.payload) !== text) return false;
				return true;
			}),
			this.client.sendMessage(this.chatId, content),
		]);
	}
}
