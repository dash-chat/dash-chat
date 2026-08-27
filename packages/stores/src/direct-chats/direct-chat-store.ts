import { reactive } from 'signalium';

import { type IMessagesClient } from '../chats/messages-client';
import { Message, MessagesStore } from '../chats/messages-store';
import { fullName } from '../contacts/contacts-client';
import { ContactReport, ContactsStore } from '../contacts/contacts-store';
import { LogsStore } from '../p2panda/logs-store';
import { SimplifiedOperation } from '../p2panda/simplified-types';
import { AgentId, DeviceId, Hash } from '../p2panda/types';
import { TombstoneStore } from '../tombstones/tombstone-store';
import { BlockEvent, ChatId, ChatSummary, Payload } from '../types';
import {
	EventWithProvenance,
	groupEventsInDays,
} from '../utils/group-events-in-days';

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
		public chatId: ChatId,
		messagesClient: IMessagesClient,
	) {
		this.messages = new MessagesStore(
			logsStore,
			contactsStore,
			tombstoneStore,
			chatId,
			messagesClient,
			reactive(async () => await this.isBlocked()),
		);
	}

	/** The established contact on the other side, if any. */
	private contact = reactive(async () => {
		const contacts = await this.contactsStore.contacts();
		return Object.values(contacts).find(
			contact => contact.chatId === this.chatId,
		);
	});

	/** The outgoing contact request this chat originated from, if any. */
	private outgoingRequest = reactive(async () => {
		const requests = await this.contactsStore.outgoingContactRequests();
		return requests.find(request => request.chatId === this.chatId);
	});

	peerAgentId = reactive(async (): Promise<AgentId | undefined> => {
		const contact = await this.contact();
		if (contact !== undefined) return contact.agentId;
		const request = await this.contactRequest();
		return request?.agentId;
	});

	isBlocked = reactive(async (): Promise<boolean> => {
		const agentId = await this.peerAgentId();
		if (agentId === undefined) return false;
		return await this.contactsStore.isBlocked(agentId);
	});

	peerProfile = reactive(async () => {
		const agentId = await this.peerAgentId();
		if (agentId === undefined) return undefined;
		const request = await this.contactRequest();
		if (request) return request.profile;
		return await this.contactsStore.profiles(agentId);
	});

	peerName = reactive(async (): Promise<string> => {
		const profile = await this.peerProfile();
		if (profile) return fullName(profile);
		const request = await this.outgoingRequest();
		return request?.profileName ?? '';
	});

	contactRequest = reactive(async () => {
		const contactRequests = await this.contactsStore.contactRequests();
		return contactRequests.find(request => request.chatId === this.chatId);
	});

	groupedEvents = reactive(async () => {
		const messages = await this.messages.messages();
		const agentId = await this.peerAgentId();
		const reports =
			agentId === undefined ? {} : await this.contactsStore.reports(agentId);
		const blockHistory =
			agentId === undefined
				? {}
				: await this.contactsStore.blockHistory(agentId);
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
		return this.logsStore.logsClient.onNewOperation((topicId, op) => {
			if (topicId !== this.chatId) return;
			if (!(op.body?.type === 'Chat' && op.body.payload.type === 'Message'))
				return;
			handler(op, op.body.payload.payload.message);
		});
	}

	summary = reactive(async (): Promise<ChatSummary> => {
		const profile = await this.peerProfile();
		const contact = await this.contact();
		const request = await this.contactRequest();
		const outgoing = await this.outgoingRequest();
		const message = await this.messages.lastMessage();
		const blocked = await this.isBlocked();

		const incomingRequest = contact === undefined ? request : undefined;
		const outgoingRequest = contact === undefined ? outgoing : undefined;
		const lastEvent: ChatSummary['lastEvent'] =
			incomingRequest !== undefined
				? {
						kind: 'contact_request',
						timestamp: message?.timestamp ?? incomingRequest.timestamp,
					}
				: message
					? {
							kind: 'message',
							content: message.content,
							timestamp: message.timestamp,
						}
					: outgoingRequest !== undefined
						? {
								kind: 'contact_request',
								timestamp: outgoingRequest.timestamp,
							}
						: {
								kind: 'contact_added',
								timestamp: contact?.addedTimestamp ?? 0,
							};

		return {
			type: 'DirectChat',
			chatId: this.chatId,
			blocked,
			name: await this.peerName(),
			avatar: profile?.avatar,
			lastEvent,
			unreadMessages: Math.max(
				await this.messages.unreadCount(),
				request !== undefined && !blocked ? 1 : 0,
			),
			waitingForProfile: profile === undefined ? true : undefined,
		};
	});
}
