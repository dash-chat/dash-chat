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
import { type IChatsClient } from './chats-client';

export class ChatsStore {
	private groupChatVersion = signal(0);

	constructor(
		protected logsStore: LogsStore<Payload>,
		protected contactsStore: ContactsStore,
		public client: IChatsClient,
		private directChatClientFactory: () => IDirectChatClient = () =>
			new DirectChatClient(),
		private groupChatClientFactory: () => IGroupChatClient = () =>
			new GroupChatClient(),
	) {
		this.logsStore.logsClient.onNewOperation((_topicId, op) => {
			if (op.body?.type === 'Chat' && op.body.payload.type === 'JoinGroup') {
				this.groupChatVersion.value++;
			}
		});
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
		this.groupChatVersion.value++;
	}

	groupChats = memo(
		(chatId: ChatId) =>
			new GroupChatStore(
				this.logsStore,
				this.contactsStore,
				this.groupChatClientFactory(),
				chatId,
			),
	);

	directChats = memo(
		(peer: AgentId) =>
			new DirectChatStore(
				this.logsStore,
				this.contactsStore,
				this.directChatClientFactory(),
				peer,
			),
	);

	allChatsIds = reactive(async () => {
		const contacts = await this.contactsStore.contactsAgentIds();
		// Combine and deduplicate
		return contacts;
	});

	allChatsSummaries = reactive(async () => {
		const [direct, groups, pending] = await Promise.all([
			this.allDirectChatSummaries(),
			this.allGroupChatSummaries(),
			this.allPendingRequestSummaries(),
		]);
		const summaries = [...direct, ...groups, ...pending];
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
					self.findIndex(r => r.code.agent_id === request.code.agent_id) ===
					index,
			);
			return unique.map(pendingRequest => ({
				type: 'DirectChat',
				chatId: pendingRequest.code.agent_id,
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
}
