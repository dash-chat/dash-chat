import { ReactivePromise, reactive } from 'signalium';

import { Profile } from '../contacts/contacts-client';
import { ContactsStore } from '../contacts/contacts-store';
import { Message } from '../direct-chats/direct-chat-store';
import { LogsStore } from '../p2panda/logs-store';
import { AgentId, PublicKey } from '../p2panda/types';
import { ChatId, MessageContent, Payload } from '../types';
import { type IGroupChatClient } from './group-chat-client';

export interface GroupInfo {
	name: string;
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
			description: 'descmygroup',
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
				content: { message: 'heeey', media: undefined },
				author: await this.contactsStore.myAgentId(),
				seqNum: 0,
				timestamp: Date.now(),
				reactions: {},
			},
		];

		return messages;
	});

	membersIds = reactive(async () => {
		const myAgentId = await this.contactsStore.myAgentId();
		return [myAgentId];
	});

	me = reactive(async () => {
		const agentId = await this.contactsStore.myAgentId();
		return await this.members(agentId);
	});

	allMembers = reactive(async () => {
		const membersIds = await this.membersIds();

		const members = await Promise.all(
			membersIds.map(memberId => this.members(memberId)),
		);

		const allMembers: Record<AgentId, GroupMember> = {};

		for (let i = 0; i < membersIds.length; i++) {
			allMembers[membersIds[i]] = members[i];
		}

		return allMembers;
	});

	members = reactive(async (agentId: AgentId) => {
		const profile = await this.contactsStore.profiles(agentId);

		const member: GroupMember = {
			agentId: agentId,
			profile,
			admin: true,
		};

		return member;
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
