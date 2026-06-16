import { invoke } from '@tauri-apps/api/core';

import { AgentId, DeviceId, Hash } from '../p2panda/types';
import { ChatId, GroupInfo, OutgoingMedia } from '../types';

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

	sendMessage(
		chatId: ChatId,
		message: string,
		media: OutgoingMedia | null,
	): Promise<void>;
	markMessagesRead(chatId: ChatId, messageHashes: Hash[]): Promise<void>;

	setInfo(chatId: ChatId, info: GroupInfo): Promise<void>;

	leaveGroup(): Promise<void>;
	deleteGroup(): Promise<void>;
}

export class GroupChatClient implements IGroupChatClient {
	async getMembers(chatId: ChatId): Promise<GroupMember[]> {
		return invoke('get_group_members', { chatId });
	}

	async addMember(chatId: ChatId, member: AgentId): Promise<void> {
		await invoke('add_group_member', { chatId, agentId: member });
	}
	async removeMember(chatId: ChatId, member: AgentId): Promise<void> {}

	sendMessage(
		chatId: ChatId,
		message: string,
		media: OutgoingMedia | null,
	): Promise<void> {
		return invoke('send_message', {
			chatId,
			message,
			media,
		});
	}
	markMessagesRead(chatId: ChatId, messageHashes: Hash[]): Promise<void> {
		return invoke('mark_messages_read', { chatId, messageHashes });
	}
	setInfo(chatId: ChatId, info: GroupInfo): Promise<void> {
		return invoke('set_group_info', { chatId, info });
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
