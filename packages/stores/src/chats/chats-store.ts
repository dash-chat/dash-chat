import { reactive } from 'signalium';

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
import { AgentId, PublicKey } from '../p2panda/types';
import { ChatId, ChatSummary, Payload } from '../types';
import { memo } from '../utils/memo';
import { type IChatsClient } from './chats-client';

export class ChatsStore {
	constructor(
		protected logsStore: LogsStore<Payload>,
		protected contactsStore: ContactsStore,
		public client: IChatsClient,
		private directChatClientFactory: () => IDirectChatClient = () =>
			new DirectChatClient(),
		private groupChatClientFactory: () => IGroupChatClient = () =>
			new GroupChatClient(),
	) {}

	async createGroup(initialMembers: PublicKey[]): Promise<GroupChatStore> {
		const chatId = await this.client.createGroup(initialMembers);
		return this.groupChats(chatId);
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
		const chatIds = await this.allChatsIds();

		let summaries = await Promise.all(
			chatIds.map(chatId => this.directChats(chatId).summary()),
		);

		const pendingRequests = await this.contactsStore.contactRequests();

		// Deduplicate by agent_id
		const uniquePendingRequests = pendingRequests.filter(
			(request, index, self) =>
				self.findIndex(r => r.code.agent_id === request.code.agent_id) ===
				index,
		);

		const pendingRequestsSummaries: ChatSummary[] = uniquePendingRequests.map(
			pendingRequest => ({
				type: 'ContactRequest',
				chatId: pendingRequest.code.agent_id,
				name: fullName(pendingRequest.profile),
				avatar: pendingRequest.profile.avatar,
				lastEvent: {
					summary: '',
					timestamp: pendingRequest.timestamp,
				},
				unreadMessages: 1,
			}),
		);

		summaries = [...summaries, ...pendingRequestsSummaries];
		summaries.sort((a, b) => b.lastEvent.timestamp - a.lastEvent.timestamp);

		return summaries;
	});
}
