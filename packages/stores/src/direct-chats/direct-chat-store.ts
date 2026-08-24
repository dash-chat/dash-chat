import { reactive } from 'signalium';

import { isPendingChatKey, pendingChatKeyDevice } from '../chats/chat-key';
import { type IMessagesClient } from '../chats/messages-client';
import { Message, MessagesStore } from '../chats/messages-store';
import { fullName } from '../contacts/contacts-client';
import { ContactReport, ContactsStore } from '../contacts/contacts-store';
import { LogsStore } from '../p2panda/logs-store';
import { SimplifiedOperation } from '../p2panda/simplified-types';
import { AgentId, DeviceId, Hash } from '../p2panda/types';
import { TombstoneStore } from '../tombstones/tombstone-store';
import { BlockEvent, ChatSummary, Payload } from '../types';
import {
	EventWithProvenance,
	groupEventsInDays,
} from '../utils/group-events-in-days';
import { type IDirectChatClient } from './direct-chat-client';

export type DirectChatEvent =
	| { kind: 'message'; message: Message }
	| { kind: 'report'; report: ContactReport }
	| { kind: 'block'; event: BlockEvent };

// Store tied to a specific direct chat
export class DirectChatStore {
	messages: MessagesStore;

	constructor(
		protected logsStore: LogsStore<Payload>,
		protected contactsStore: ContactsStore,
		protected tombstoneStore: TombstoneStore,
		public client: IDirectChatClient,
		public peer: AgentId,
		messagesClient: IMessagesClient,
	) {
		this.messages = new MessagesStore(
			logsStore,
			contactsStore,
			tombstoneStore,
			this.chatId,
			messagesClient,
			this.agentIdForDeviceId,
		);
	}

	get isPending(): boolean {
		return isPendingChatKey(this.peer);
	}

	resolvedPendingAgent = reactive(async () => {
		const device = pendingChatKeyDevice(this.peer);
		if (device === undefined) return undefined;
		// Depend on the reactive contacts list so this re-runs once the contact
		// is established (the device→agent mapping is saved around the same time
		// the AddContact marker is published).
		await this.contactsStore.contactsAgentIds();
		return await this.contactsStore.client.agentForDevice(device);
	});

	chatId = reactive(async () => {
		if (this.isPending) return '';
		return await this.client.chatId(this.peer);
	});

	peerProfile = reactive(async () => {
		if (this.isPending) return undefined;
		const request = await this.contactRequest();
		if (request) return request.profile;
		return await this.contactsStore.profiles(this.peer);
	});

	peerName = reactive(async (): Promise<string> => {
		const profile = await this.peerProfile();
		if (profile) return fullName(profile);
		if (!this.isPending) return '';
		const pending = (await this.contactsStore.outgoingPendingRequests()).find(
			request => request.devicePubkey === pendingChatKeyDevice(this.peer),
		);
		return pending?.profileName ?? '';
	});

	contactRequest = reactive(async () => {
		const contactRequests = await this.contactsStore.contactRequests();
		return contactRequests.find(cr => cr.agentId === this.peer);
	});

	agentIdForDeviceId = reactive(async (deviceId: DeviceId) => {
		const myDeviceId = await this.contactsStore.myDeviceId();
		if (deviceId === myDeviceId) return await this.contactsStore.myAgentId();
		return this.peer;
	});

	groupedEvents = reactive(async () => {
		const messages = await this.messages.messages();
		const reports = this.isPending
			? {}
			: await this.contactsStore.reports(this.peer);
		const blockHistory = this.isPending
			? {}
			: await this.contactsStore.blockHistory(this.peer);
		const peerName = await this.peerName();

		const eventsWithProvenance: Record<
			Hash,
			EventWithProvenance<DirectChatEvent>
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

		for (const [hash, report] of Object.entries(reports)) {
			devices.add(report.author);
			eventsWithProvenance[hash] = {
				event: { kind: 'report', report },
				author: report.author,
				timestamp: report.timestamp,
				type: 'ReportContact',
			};
		}

		for (const [hash, block] of Object.entries(blockHistory)) {
			devices.add(block.author);
			eventsWithProvenance[hash] = {
				event: {
					kind: 'block',
					event: {
						kind: block.blocked ? 'contact_blocked' : 'contact_unblocked',
						contactName: peerName === '' ? undefined : peerName,
						timestamp: block.timestamp,
					},
				},
				author: block.author,
				timestamp: block.timestamp,
				type: 'Block',
			};
		}

		const agentsSets = Array.from(devices).map(a => [a]);

		return groupEventsInDays(eventsWithProvenance, agentsSets);
	});

	onNewMessage(
		handler: (operation: SimplifiedOperation<Payload>, message: string) => void,
	) {
		return this.logsStore.logsClient.onNewOperation(async (topicId, op) => {
			const chatId = await this.chatId();
			if (topicId !== chatId) return;
			if (!(op.body?.type === 'Chat' && op.body.payload.type === 'Message'))
				return;
			handler(op, op.body.payload.payload.message);
		});
	}

	summary = reactive(async (): Promise<ChatSummary> => {
		const profile = await this.peerProfile();
		const message = await this.messages.lastMessage();
		const unreadCount = await this.messages.unreadCount();

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
			name: await this.peerName(),
			avatar: profile?.avatar,
			lastEvent,
			unreadMessages: unreadCount,
		};
	});
}
