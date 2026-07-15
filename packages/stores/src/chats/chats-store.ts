import { reactive, signal } from 'signalium';

import { fullName } from '../contacts/contacts-client';
import { ContactsStore } from '../contacts/contacts-store';
import {
	DirectChatClient,
	type IDirectChatClient,
} from '../direct-chats/direct-chat-client';
import { DirectChatStore } from '../direct-chats/direct-chat-store';
import {
	GroupChatClient,
	type IGroupChatClient,
} from '../group-chats/group-chat-client';
import { GroupChatStore } from '../group-chats/group-chat-store';
import { LogsStore } from '../p2panda/logs-store';
import { AgentId, VerifyingKey } from '../p2panda/types';
import { ChatId, ChatSummary, Payload } from '../types';
import { memo } from '../utils/memo';
import { pendingChatKey } from './chat-key';
import { type IChatsClient } from './chats-client';
import { type IMessagesClient, MessagesClient } from './messages-client';

export class ChatsStore {
	private groupChatVersion = signal(0);

	constructor(
		protected logsStore: LogsStore<Payload>,
		protected contactsStore: ContactsStore,
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

	protected directChatClient(): IDirectChatClient {
		return new DirectChatClient();
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
				this.groupChatClient(),
				chatId,
				this.messagesClient(),
			),
	);

	directChats = memo(
		(peer: AgentId) =>
			new DirectChatStore(
				this.logsStore,
				this.contactsStore,
				this.directChatClient(),
				peer,
				this.messagesClient(),
			),
	);

	allChatsIds = reactive(async () => {
		const contacts = await this.contactsStore.contactsAgentIds();
		// Combine and deduplicate
		return contacts;
	});

	allChatsSummaries = reactive(async () => {
		const [direct, groups, pending, outgoing] = await Promise.all([
			this.allDirectChatSummaries(),
			this.allGroupChatSummaries(),
			this.allPendingRequestSummaries(),
			this.allOutgoingPendingSummaries(),
		]);
		const summaries = [...direct, ...groups, ...pending, ...outgoing];
		summaries.sort((a, b) => b.lastEvent.timestamp - a.lastEvent.timestamp);
		return summaries;
	});

	private allDirectChatSummaries = reactive(async () => {
		const chatIds = await this.allChatsIds();
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

	private allPendingRequestSummaries = reactive(
		async (): Promise<ChatSummary[]> => {
			const pendingRequests = await this.contactsStore.contactRequests();
			const unique = pendingRequests.filter(
				(request, index, self) =>
					self.findIndex(r => r.agentId === request.agentId) === index,
			);
			return unique.map(pendingRequest => ({
				type: 'DirectChat',
				chatId: pendingRequest.agentId,
				name: fullName(pendingRequest.profile),
				avatar: pendingRequest.profile.avatar,
				lastEvent: {
					kind: 'contact_request',
					timestamp: pendingRequest.timestamp,
				},
				unreadMessages: 1,
			}));
		},
	);

	private allOutgoingPendingSummaries = reactive(
		async (): Promise<ChatSummary[]> => {
			const pending = await this.contactsStore.outgoingPendingRequests();
			return pending.map(request => ({
				type: 'DirectChat',
				chatId: pendingChatKey(request.devicePubkey),
				name: '',
				avatar: undefined,
				waitingForProfile: true as const,
				lastEvent: {
					kind: 'contact_request',
					timestamp: request.timestamp,
				},
				unreadMessages: 0,
			}));
		},
	);
}
