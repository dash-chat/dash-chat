import { reactive, signal } from 'signalium';

import { type IMessagesClient } from '../chats/messages-client';
import { Message, MessagesStore } from '../chats/messages-store';
import { Profile, fullName } from '../contacts/contacts-client';
import { ContactsStore } from '../contacts/contacts-store';
import { MessageAckStore } from '../message-acks/message-ack-store';
import { LogsStore } from '../p2panda/logs-store';
import {
	AuthGroupMember,
	GroupAction,
	SimplifiedOperation,
} from '../p2panda/simplified-types';
import { AgentId, DeviceId, Hash, VerifyingKey } from '../p2panda/types';
import { TombstoneStore } from '../tombstones/tombstone-store';
import {
	ChatId,
	ChatSummary,
	ChatSummaryLastEvent,
	GroupControlEvent,
	GroupInfo,
	Payload,
} from '../types';
import {
	EventWithProvenance,
	groupEventsInDays,
} from '../utils/group-events-in-days';
import { type IGroupChatClient } from './group-chat-client';

export type ChatEvent =
	| { kind: 'message'; message: Message }
	| { kind: 'control'; event: GroupControlEvent };

export interface GroupMemberWithProfile {
	agentId: AgentId;
	deviceIds: DeviceId[];
	profile: Profile | undefined;
	admin: boolean;
	member: boolean;
}

export class GroupChatStore {
	private membersVersion = signal(0);

	messages: MessagesStore;

	constructor(
		protected logsStore: LogsStore<Payload>,
		protected contactsStore: ContactsStore,
		protected tombstoneStore: TombstoneStore,
		messageAckStore: MessageAckStore,
		public client: IGroupChatClient,
		public chatId: ChatId,
		messagesClient: IMessagesClient,
	) {
		this.messages = new MessagesStore(
			logsStore,
			contactsStore,
			tombstoneStore,
			messageAckStore,
			chatId,
			messagesClient,
			reactive(async () => !(await this.me()).member),
		);
		this.logsStore.logsClient.onNewOperation((topicId, op) => {
			if (topicId === this.chatId && op.header.auth) {
				this.membersVersion.value++;
			}
		});
	}

	info = reactive(async () => {
		const logs = await this.logsStore.logsForAllAuthors(this.chatId);

		let latest:
			| { info: GroupInfo; timestamp: number; seqNum: number }
			| undefined;
		for (const operations of Object.values(logs)) {
			for (const operation of operations) {
				const body = operation.body;
				if (body?.type !== 'Chat') continue;
				if (body.payload.type !== 'GroupInfo') continue;
				const timestamp = operation.header.timestamp;
				const seqNum = operation.header.seq_num;
				if (
					!latest ||
					timestamp > latest.timestamp ||
					(timestamp === latest.timestamp && seqNum > latest.seqNum)
				) {
					latest = { info: body.payload.payload, timestamp, seqNum };
				}
			}
		}

		const info: GroupInfo = latest?.info ?? {
			name: '',
			description: undefined,
			image: undefined,
		};
		return info;
	});

	/** Members who left or were removed are gone from the member list, so fall
	 * back to the device→agent mapping, which covers non-contacts too: every
	 * group member is introduced to the group when they join. */
	private nameForDevice = reactive(
		async (deviceId: DeviceId): Promise<string | undefined> => {
			const members = await this.allMembers();
			const name = findName(members, deviceId);
			if (name) return name;
			const agentId = await this.contactsStore.agentForDevice(deviceId);
			if (!agentId) return undefined;
			const profile = await this.contactsStore.profiles(agentId);
			return profile ? fullName(profile) : undefined;
		},
	);

	controlEvents = reactive(async () => {
		const logs = await this.logsStore.logsForAllAuthors(this.chatId);
		const myDeviceId = await this.contactsStore.myDeviceId();

		const events: Record<Hash, GroupControlEvent> = {};
		for (const op of Object.values(logs).flat()) {
			const event = await buildGroupControlEvent(
				op,
				myDeviceId,
				this.nameForDevice,
			);
			if (event) events[op.hash] = event;
		}
		return events;
	});

	groupedEvents = reactive(async () => {
		const messages = await this.messages.messages();
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
			for (const op of ops) opAuthorByHash[op.hash] = op.header.verifying_key;
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

		return groupEventsInDays(eventsWithProvenance, agentsSets);
	});

	lastEvent = reactive(async (): Promise<ChatSummaryLastEvent | undefined> => {
		const controlEvents = await this.controlEvents();
		const lastMessage = await this.messages.lastMessage();

		let bestAuth: GroupControlEvent | undefined;
		let createdByMe = false;
		let iWasAdded = false;
		for (const event of Object.values(controlEvents)) {
			if (event.kind === 'group_created') {
				if (event.isMine) createdByMe = true;
				if (event.iAmInitialMember) iWasAdded = true;
			} else if (event.kind === 'group_member_added' && event.isMine) {
				iWasAdded = true;
			}
			if (!bestAuth || event.timestamp > bestAuth.timestamp) {
				bestAuth = event;
			}
		}

		// Hide the group until we either authored the Create or see our own Add op.
		// Avoids the chat-list flicker between empty → "Group created" → "X added you".
		if (!createdByMe && !iWasAdded) return undefined;

		const messageEvent: ChatSummaryLastEvent | undefined = lastMessage
			? {
					kind: 'message',
					content: lastMessage.content,
					authorName: await this.nameForDevice(lastMessage.author),
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
			entry !== undefined,
		);
	});

	allMembers = reactive(async () => {
		const data = await this.membersData();
		const entries = await Promise.all(
			data.map(async ({ agentId, deviceIds, isAdmin }) => {
				const member = await this.buildMember(
					agentId,
					deviceIds,
					isAdmin,
					true,
				);
				return [agentId, member] as const;
			}),
		);
		return Object.fromEntries(entries) as Record<
			AgentId,
			GroupMemberWithProfile
		>;
	});

	private buildMember = reactive(
		async (
			agentId: AgentId,
			deviceIds: DeviceId[],
			admin: boolean,
			member: boolean,
		) => {
			const profile = await this.contactsStore.profiles(agentId);
			return {
				agentId,
				deviceIds,
				profile,
				admin,
				member,
			} satisfies GroupMemberWithProfile;
		},
	);

	summary = reactive(async (): Promise<ChatSummary | undefined> => {
		const last = await this.lastEvent();
		if (!last) return undefined;
		const info = await this.info();
		const unread = await this.messages.unreadCount();

		return {
			type: 'GroupChat',
			chatId: this.chatId,
			name: info.name,
			avatar: info.image,
			lastEvent: last,
			unreadMessages: unread,
		};
	});

	/// Actions

	async addMembers(members: VerifyingKey[]) {
		await Promise.all(
			members.map(member => this.client.addMember(this.chatId, member)),
		);
		this.membersVersion.value++;
	}

	async setInfo(info: GroupInfo): Promise<void> {
		await this.client.setInfo(this.chatId, info);
	}
}

function findName(
	members: Record<AgentId, GroupMemberWithProfile>,
	deviceId: DeviceId,
): string | undefined {
	for (const m of Object.values(members)) {
		if (m.deviceIds.includes(deviceId)) {
			return m.profile ? fullName(m.profile) : undefined;
		}
	}
	return undefined;
}

/** The device a membership action names, or undefined for an action that
 * targets a subgroup rather than an individual. */
function memberOfAuthAction(action: GroupAction): DeviceId | undefined {
	if ('Add' in action) return individual(action.Add.member);
	if ('Remove' in action) return individual(action.Remove.member);
	if ('Promote' in action) return individual(action.Promote.member);
	if ('Demote' in action) return individual(action.Demote.member);
	return undefined;
}

function individual(member: AuthGroupMember): DeviceId | undefined {
	return 'Individual' in member ? member.Individual : undefined;
}

async function buildGroupControlEvent(
	op: SimplifiedOperation<Payload>,
	myDeviceId: DeviceId,
	nameForDevice: (deviceId: DeviceId) => Promise<string | undefined>,
): Promise<GroupControlEvent | undefined> {
	const action = op.header.auth?.action;
	if (!action) return undefined;
	const ts = op.header.timestamp;
	const actorDeviceId = op.header.verifying_key;
	const actorIsMe = actorDeviceId === myDeviceId;

	if ('Create' in action) {
		const iAmInitialMember = action.Create.initial_members.some(
			([m]) => 'Individual' in m && m.Individual === myDeviceId,
		);
		return {
			kind: 'group_created',
			isMine: actorIsMe,
			iAmInitialMember,
			creatorName: actorIsMe ? undefined : await nameForDevice(actorDeviceId),
			timestamp: ts,
		};
	}
	const memberDeviceId = memberOfAuthAction(action);
	const memberName = memberDeviceId
		? await nameForDevice(memberDeviceId)
		: undefined;
	const adminName = await nameForDevice(actorDeviceId);
	const targetsMe = memberDeviceId === myDeviceId;

	if ('Add' in action) {
		return {
			kind: 'group_member_added',
			isMine: targetsMe,
			addedByMe: actorIsMe,
			memberName,
			adminName,
			timestamp: ts,
		};
	}
	if ('Remove' in action) {
		if (memberDeviceId === actorDeviceId) {
			return {
				kind: 'group_member_left',
				isMine: actorIsMe,
				memberName,
				timestamp: ts,
			};
		}
		return {
			kind: 'group_member_removed',
			isMine: targetsMe,
			removedByMe: actorIsMe,
			memberName,
			adminName,
			timestamp: ts,
		};
	}
	if ('Promote' in action) {
		return {
			kind: 'group_member_promoted',
			promotedByMe: actorIsMe,
			memberName,
			adminName,
			timestamp: ts,
		};
	}
	if ('Demote' in action) {
		return {
			kind: 'group_member_demoted',
			demotedByMe: actorIsMe,
			memberName,
			adminName,
			timestamp: ts,
		};
	}
	return undefined;
}
