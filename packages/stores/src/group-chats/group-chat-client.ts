import { invoke } from '@tauri-apps/api/core';

import { AgentId, DeviceId } from '../p2panda/types';
import { ChatId, MessageContent } from '../types';

export interface GroupMember {
	agentId: AgentId;
	deviceIds: DeviceId[];
	isAdmin: boolean;
}

export interface IGroupChatClient {
	getMembers(chatId: ChatId): Promise<GroupMember[]>;
	addMember(chatId: ChatId, member: AgentId): Promise<void>;
	removeMember(chatId: ChatId, member: AgentId): Promise<void>;

	promoteToAdministrator(chatId: ChatId, member: AgentId): Promise<void>;
	demoteFromAdministrator(chatId: ChatId, member: AgentId): Promise<void>;

	sendMessage(chatId: ChatId, content: MessageContent): Promise<void>;

	leaveGroup(): Promise<void>;
	deleteGroup(): Promise<void>;
}

export class GroupChatClient implements IGroupChatClient {
	async getMembers(chatId: ChatId): Promise<GroupMember[]> {
		return invoke('get_group_members', { chatId });
	}

	async addMember(chatId: ChatId, member: AgentId): Promise<void> {
		throw new Error('addMember not implemented');
	}
	async removeMember(chatId: ChatId, member: AgentId): Promise<void> {}

	sendMessage(chatId: ChatId, content: MessageContent): Promise<void> {
		return invoke('send_message', { chatId, content });
	}
	async promoteToAdministrator(
		chatId: ChatId,
		member: AgentId,
	): Promise<void> {}
	async demoteFromAdministrator(
		chatId: ChatId,
		member: AgentId,
	): Promise<void> {}

	async leaveGroup(): Promise<void> {}

	async deleteGroup(): Promise<void> {}
}
