import { ReactivePromise, reactive } from 'signalium';

import { Profile } from '../contacts/contacts-client';
import { ContactsStore } from '../contacts/contacts-store';
import { Message } from '../direct-chats/direct-chat-store';
import { LogsStore } from '../p2panda/logs-store';
import { AgentId, PublicKey } from '../p2panda/types';
import { ChatId, ChatSummary, MessageContent, Payload } from '../types';
import {
	type GroupMemberData,
	type IGroupChatClient,
} from './group-chat-client';

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
		// const allLogs = await this.logsStore.logsForAllAuthors(this.chatId);
		// const messages = await invoke('get_messages', {
		// 	chatId: this.chatId,
		// });
		// const messages : Array<SimplifiedOperation<ChatMessageContent>> = [{
		// 	hash: '',
		// 	header: {

		// 	}
		// }]
		const messages: Array<Message> = [
			{
				hash: '123',
				content:
					"This is a dummy first message. Real group messaging isn't implemented yet.",
				author: await this.contactsStore.myAgentId(),
				seqNum: 0,
				timestamp: Date.now(),
				reactions: {},
			},
		];

		return messages;
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
		const allMembers: Record<AgentId, GroupMember> = {};
		for (const { agentId, isAdmin } of data) {
			allMembers[agentId] = await this.buildMember(agentId, isAdmin);
		}
		return allMembers;
	});

	private buildMember = reactive(async (agentId: AgentId, admin: boolean) => {
		const profile = await this.contactsStore.profiles(agentId);
		return { agentId, profile, admin } satisfies GroupMember;
	});

	summary = reactive(async (): Promise<ChatSummary> => {
		const info = await this.info();
		const messages = await this.messages();
		const lastMessage = messages[messages.length - 1];

		const lastEvent = lastMessage
			? { summary: lastMessage.content, timestamp: lastMessage.timestamp }
			: { summary: '', timestamp: 0 };

		return {
			type: 'GroupChat',
			chatId: this.chatId,
			name: info.name ?? '',
			avatar: info.avatar,
			lastEvent,
			unreadMessages: 0,
		};
	});

	/// Actions

	addMember(member: PublicKey) {
		return this.client.addMember(this.chatId, member);
	}

	sendMessage(text: string) {
		const content: MessageContent = { v: '1', message: text, media: null };
		return this.client.sendMessage(this.chatId, content);
	}
}
