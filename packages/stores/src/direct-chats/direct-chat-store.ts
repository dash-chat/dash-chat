import { reactive } from 'signalium';

import { isPendingChatKey, pendingChatKeyDevice } from '../chats/chat-key';
import { type IMessagesClient } from '../chats/messages-client';
import { Message, MessagesStore } from '../chats/messages-store';
import { fullName } from '../contacts/contacts-client';
import { ContactsStore } from '../contacts/contacts-store';
import { LogsStore } from '../p2panda/logs-store';
import { SimplifiedOperation } from '../p2panda/simplified-types';
import { AgentId, DeviceId, Hash } from '../p2panda/types';
import { TombstoneStore } from '../tombstones/tombstone-store';
import { ChatSummary, Payload } from '../types';
import {
	EventWithProvenance,
	groupEventsInDays,
} from '../utils/group-events-in-days';
import { type IDirectChatClient } from './direct-chat-client';

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

	contactRequest = reactive(async () => {
		const contactRequests = await this.contactsStore.contactRequests();
		return contactRequests.find(cr => cr.agentId === this.peer);
	});

	groupedMessages = reactive(async () => {
		const messages = await this.messages.messages();

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

		const messagesWithProvenance = groupEventsInDays(
			eventsWithProvenance,
			agentsSets,
		);
		return messagesWithProvenance;
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

		const pendingName = this.isPending
			? (await this.contactsStore.outgoingPendingRequests()).find(
					request => request.devicePubkey === pendingChatKeyDevice(this.peer),
				)?.profileName
			: undefined;

		return {
			type: 'DirectChat',
			chatId: this.peer,
			name: profile ? fullName(profile) : (pendingName ?? ''),
			avatar: profile?.avatar,
			lastEvent,
			unreadMessages: unreadCount,
		};
	});
}
