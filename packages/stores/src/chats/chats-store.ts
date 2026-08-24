import { reactive, signal } from 'signalium';

import { ContactsStore } from '../contacts/contacts-store';
import { DirectChatStore } from '../direct-chats/direct-chat-store';
import {
	GroupChatClient,
	type IGroupChatClient,
} from '../group-chats/group-chat-client';
import { GroupChatStore } from '../group-chats/group-chat-store';
import { LogsStore } from '../p2panda/logs-store';
import { VerifyingKey } from '../p2panda/types';
import { TombstoneStore } from '../tombstones/tombstone-store';
import { ChatId, ChatSummary, Payload } from '../types';
import { memo } from '../utils/memo';
import { type IChatsClient } from './chats-client';
import { type IMessagesClient, MessagesClient } from './messages-client';

export class ChatsStore {
	private groupChatVersion = signal(0);

	constructor(
		protected logsStore: LogsStore<Payload>,
		protected contactsStore: ContactsStore,
		protected tombstoneStore: TombstoneStore,
		public client: IChatsClient,
	) {
		this.logsStore.logsClient.onNewOperation((_topicId, op) => {
			// GroupControl bumps are what reveal a newly joined group: the backend
			// marks a chat as a group chat while reducing the group's Create op
			// (before emitting this notification), which happens after the
			// JoinGroup notification has already triggered a (too early) refetch.
			if (
				(op.body?.type === 'Chat' && op.body.payload.type === 'JoinGroup') ||
				op.body?.type === 'GroupControl'
			) {
				this.groupChatVersion.value++;
			}
		});
	}

	protected groupChatClient(): IGroupChatClient {
		return new GroupChatClient();
	}

	protected messagesClient(): IMessagesClient {
		return new MessagesClient();
	}

	private groupChatIds = reactive(async () => {
		void this.groupChatVersion.value;
		try {
			return await this.client.getGroupChats();
		} catch (err) {
			console.error('Failed to fetch group chats', err);
			throw err;
		}
	});

	async createGroup(initialMembers: VerifyingKey[]): Promise<GroupChatStore> {
		const chatId = await this.client.createGroup(initialMembers);
		this.groupChatVersion.value++;
		return this.groupChats(chatId);
	}

	async leaveGroup(chatId: ChatId): Promise<void> {
		await this.groupChats(chatId).client.leaveGroup(chatId);
	}

	groupChats = memo(
		(chatId: ChatId) =>
			new GroupChatStore(
				this.logsStore,
				this.contactsStore,
				this.tombstoneStore,
				this.groupChatClient(),
				chatId,
				this.messagesClient(),
			),
	);

	directChats = memo(
		(chatId: ChatId) =>
			new DirectChatStore(
				this.logsStore,
				this.contactsStore,
				this.tombstoneStore,
				chatId,
				this.messagesClient(),
			),
	);

	allChatsSummaries = reactive(async () => {
		const [direct, groups] = await Promise.all([
			this.allDirectChatSummaries(),
			this.allGroupChatSummaries(),
		]);
		const summaries = [...direct, ...groups];
		summaries.sort((a, b) => b.lastEvent.timestamp - a.lastEvent.timestamp);
		return summaries;
	});

	/** Every direct chat, whatever its lifecycle state: established contacts,
	 * incoming contact requests, and outgoing requests awaiting their ack. */
	private allDirectChatIds = reactive(async (): Promise<ChatId[]> => {
		const [contacts, requests, outgoing] = await Promise.all([
			this.contactsStore.contacts(),
			this.contactsStore.contactRequests(),
			this.contactsStore.outgoingContactRequests(),
		]);
		const chatIds = new Set<ChatId>();
		for (const contact of Object.values(contacts)) chatIds.add(contact.chatId);
		for (const request of requests) chatIds.add(request.chatId);
		for (const request of outgoing) chatIds.add(request.chatId);
		return Array.from(chatIds);
	});

	private allDirectChatSummaries = reactive(async () => {
		const chatIds = await this.allDirectChatIds();
		return Promise.all(
			chatIds.map(chatId => this.directChats(chatId).summary()),
		);
	});

	private allGroupChatSummaries = reactive(async () => {
		const groupChatIds = await this.groupChatIds();
		const summaries = await Promise.all(
			groupChatIds.map(chatId => this.groupChats(chatId).summary()),
		);
		return summaries.filter((s): s is ChatSummary => s !== undefined);
	});
}
