import { reactive, signal } from 'signalium';

import { Profile, fullName } from '../contacts/contacts-client';
import { ContactsStore } from '../contacts/contacts-store';
import { Message } from '../direct-chats/direct-chat-store';
import { waitForOperation } from '../p2panda/logs-client';
import { LogsStore } from '../p2panda/logs-store';
import { SimplifiedOperation } from '../p2panda/simplified-types';
import { AgentId, DeviceId, Hash, PublicKey } from '../p2panda/types';
import {
	ChatId,
	ChatSummary,
	ChatSummaryLastEvent,
	GroupControlEvent,
	MessageContent,
	Payload,
	ReadMessagesStore,
	getMessageText,
} from '../types';
import { EventWithProvenance, orderInEventSets } from '../utils/event-sets';
import { type IGroupChatClient } from './group-chat-client';

export type ChatEvent =
	| { kind: 'message'; message: Message }
	| { kind: 'control'; event: GroupControlEvent };

export interface GroupInfo {
	name: string | undefined;
	description: string | undefined;
	avatar: string | undefined;
}

export interface GroupMemberWithProfile {
	agentId: AgentId;
	deviceIds: DeviceId[];
	profile: Profile | undefined;
	admin: boolean;
}

export class GroupChatStore implements ReadMessagesStore {
	private membersVersion = signal(0);

	constructor(
		protected logsStore: LogsStore<Payload>,
		protected contactsStore: ContactsStore,
		public client: IGroupChatClient,
		public chatId: ChatId,
	) {
		this.logsStore.logsClient.onNewOperation((topicId, op) => {
			if (topicId === this.chatId && op.header.auth) {
				this.membersVersion.value++;
			}
		});
	}

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

	controlEvents = reactive(async () => {
		const logs = await this.logsStore.logsForAllAuthors(this.chatId);
		const members = await this.allMembers();
		const myDeviceId = await this.contactsStore.myDeviceId();

		const nameForDevice = (deviceId: DeviceId): string => {
			for (const m of Object.values(members)) {
				if (m.deviceIds.includes(deviceId)) {
					return m.profile ? fullName(m.profile) : '';
				}
			}
			return '';
		};

		const events: Record<Hash, GroupControlEvent> = {};
		for (const ops of Object.values(logs)) {
			for (const op of ops) {
				const event = buildGroupControlEvent(op, myDeviceId, nameForDevice);
				if (event) events[op.hash] = event;
			}
		}
		return events;
	});

	messageSets = reactive(async () => {
		const messages = await this.messages();
		const controlEvents = await this.controlEvents();

		const eventsWithProvenance: Record<
			Hash,
			EventWithProvenance<ChatEvent>
		> = {};
		const devices = new Set<DeviceId>();

		for (const [hash, message] of Object.entries(messages)) {
			devices.add(message.author);
			eventsWithProvenance[hash] = {
				event: { kind: 'message', message },
				author: message.author,
				timestamp: message.timestamp,
				type: 'Message',
			};
		}

		const logs = await this.logsStore.logsForAllAuthors(this.chatId);
		const opAuthorByHash: Record<Hash, DeviceId> = {};
		for (const ops of Object.values(logs)) {
			for (const op of ops) opAuthorByHash[op.hash] = op.header.public_key;
		}

		for (const [hash, event] of Object.entries(controlEvents)) {
			const author = opAuthorByHash[hash];
			if (!author) continue;
			devices.add(author);
			eventsWithProvenance[hash] = {
				event: { kind: 'control', event },
				author,
				timestamp: event.timestamp,
				type: 'GroupControl',
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

	lastEvent = reactive(async (): Promise<ChatSummaryLastEvent | undefined> => {
		const logs = await this.logsStore.logsForAllAuthors(this.chatId);
		const lastMessage = await this.lastMessage();
		const members = await this.allMembers();
		const myDeviceId = await this.contactsStore.myDeviceId();

		const nameForDevice = (deviceId: DeviceId): string => {
			for (const m of Object.values(members)) {
				if (m.deviceIds.includes(deviceId)) {
					return m.profile ? fullName(m.profile) : '';
				}
			}
			return '';
		};

		let bestAuth: GroupControlEvent | undefined;
		let createdByMe = false;
		let iWasAdded = false;
		for (const ops of Object.values(logs)) {
			for (const op of ops) {
				const action = op.header.auth?.action;
				if (!action) continue;
				if ('Create' in action) {
					if (op.header.public_key === myDeviceId) createdByMe = true;
					const initiallyIncludedMe = action.Create.initial_members.some(
						([m]) => 'Individual' in m && m.Individual === myDeviceId,
					);
					if (initiallyIncludedMe) iWasAdded = true;
				} else if ('Add' in action) {
					const member = action.Add.member;
					const addedDeviceId =
						'Individual' in member ? member.Individual : undefined;
					if (addedDeviceId === myDeviceId) iWasAdded = true;
				}

				const candidate = buildGroupControlEvent(op, myDeviceId, nameForDevice);
				if (
					candidate &&
					(!bestAuth || candidate.timestamp > bestAuth.timestamp)
				) {
					bestAuth = candidate;
				}
			}
		}

		// Hide the group until we either authored the Create or see our own Add op.
		// Avoids the chat-list flicker between empty → "Group created" → "X added you".
		if (!createdByMe && !iWasAdded) return undefined;

		const messageEvent: ChatSummaryLastEvent | undefined = lastMessage
			? {
					kind: 'message',
					text: lastMessage.content,
					authorName: nameForDevice(lastMessage.author),
					timestamp: lastMessage.timestamp,
				}
			: undefined;

		if (!messageEvent) return bestAuth;
		if (!bestAuth) return messageEvent;
		return messageEvent.timestamp > bestAuth.timestamp
			? messageEvent
			: bestAuth;
	});

	membersData = reactive(async () => {
		void this.membersVersion.value;
		return await this.client.getMembers(this.chatId);
	});

	me = reactive(async () => {
		const myAgentId = await this.contactsStore.myAgentId();
		const data = await this.membersData();
		const entry = data.find(m => m.agentId === myAgentId);
		return this.buildMember(
			myAgentId,
			entry?.deviceIds ?? [],
			entry?.isAdmin ?? false,
		);
	});

	allMembers = reactive(async () => {
		const data = await this.membersData();
		const entries = await Promise.all(
			data.map(async ({ agentId, deviceIds, isAdmin }) => {
				const member = await this.buildMember(agentId, deviceIds, isAdmin);
				return [agentId, member] as const;
			}),
		);
		return Object.fromEntries(entries) as Record<
			AgentId,
			GroupMemberWithProfile
		>;
	});

	private buildMember = reactive(
		async (agentId: AgentId, deviceIds: DeviceId[], admin: boolean) => {
			const profile = await this.contactsStore.profiles(agentId);
			return {
				agentId,
				deviceIds,
				profile,
				admin,
			} satisfies GroupMemberWithProfile;
		},
	);

	readMessageHashes = reactive(async () => {
		const myDeviceGroupTopic =
			await this.contactsStore.devicesStore.myDeviceGroupTopic();
		const readHashes: Set<Hash> = new Set();

		for (const [_, ops] of Object.entries(myDeviceGroupTopic)) {
			for (const op of ops) {
				if (
					op.body?.payload?.type === 'ReadMessages' &&
					op.body.payload.payload.chat_id === this.chatId
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
			if (message.author !== myDeviceId && !readHashes.has(hash)) {
				count++;
			}
		}
		return count;
	});

	summary = reactive(async (): Promise<ChatSummary | undefined> => {
		const last = await this.lastEvent();
		if (!last) return undefined;
		const info = await this.info();
		const unread = await this.unreadCount();

		return {
			type: 'GroupChat',
			chatId: this.chatId,
			name: info.name ?? '',
			avatar: info.avatar,
			lastEvent: last,
			unreadMessages: unread,
		};
	});

	/// Actions

	async addMembers(members: PublicKey[]) {
		await Promise.all(
			members.map(member => this.client.addMember(this.chatId, member)),
		);
		this.membersVersion.value++;
	}

	async markAsRead(messageHashes: Hash[]): Promise<void> {
		await this.client.markMessagesRead(this.chatId, messageHashes);
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

function buildGroupControlEvent(
	op: SimplifiedOperation<Payload>,
	myDeviceId: DeviceId,
	nameForDevice: (deviceId: DeviceId) => string,
): GroupControlEvent | undefined {
	const action = op.header.auth?.action;
	if (!action) return undefined;
	const ts = op.header.timestamp;
	const actorDeviceId = op.header.public_key;
	const actorIsMe = actorDeviceId === myDeviceId;

	if ('Create' in action) {
		return {
			kind: 'group_created',
			isMine: actorIsMe,
			creatorName: actorIsMe ? '' : nameForDevice(actorDeviceId),
			timestamp: ts,
		};
	}
	if ('Add' in action) {
		const memberDeviceId =
			'Individual' in action.Add.member
				? action.Add.member.Individual
				: undefined;
		return {
			kind: 'group_member_added',
			isMine: !!memberDeviceId && memberDeviceId === myDeviceId,
			addedByMe: actorIsMe,
			memberName: memberDeviceId ? nameForDevice(memberDeviceId) : '',
			adminName: nameForDevice(actorDeviceId),
			timestamp: ts,
		};
	}
	if ('Remove' in action) {
		const memberDeviceId =
			'Individual' in action.Remove.member
				? action.Remove.member.Individual
				: undefined;
		return {
			kind: 'group_member_removed',
			isMine: !!memberDeviceId && memberDeviceId === myDeviceId,
			removedByMe: actorIsMe,
			memberName: memberDeviceId ? nameForDevice(memberDeviceId) : '',
			adminName: nameForDevice(actorDeviceId),
			timestamp: ts,
		};
	}
	if ('Promote' in action) {
		const memberDeviceId =
			'Individual' in action.Promote.member
				? action.Promote.member.Individual
				: undefined;
		return {
			kind: 'group_member_promoted',
			promotedByMe: actorIsMe,
			memberName: memberDeviceId ? nameForDevice(memberDeviceId) : '',
			adminName: nameForDevice(actorDeviceId),
			timestamp: ts,
		};
	}
	if ('Demote' in action) {
		const memberDeviceId =
			'Individual' in action.Demote.member
				? action.Demote.member.Individual
				: undefined;
		return {
			kind: 'group_member_demoted',
			demotedByMe: actorIsMe,
			memberName: memberDeviceId ? nameForDevice(memberDeviceId) : '',
			adminName: nameForDevice(actorDeviceId),
			timestamp: ts,
		};
	}
	return undefined;
}
