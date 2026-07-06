import { invoke } from '@tauri-apps/api/core';

import { AgentId, DeviceId, Hash } from '../p2panda/types';
import { ChatId, ChatReaction, GroupDetails, MessageContent } from '../types';

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
	markMessagesRead(chatId: ChatId, messageHashes: Hash[]): Promise<void>;
	sendReaction(chatId: ChatId, content: ChatReaction): Promise<void>;

	setDetails(chatId: ChatId, details: GroupDetails): Promise<void>;

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

	sendMessage(chatId: ChatId, content: MessageContent): Promise<void> {
		return invoke('send_message', { chatId, content });
	}
	markMessagesRead(chatId: ChatId, messageHashes: Hash[]): Promise<void> {
		return invoke('mark_messages_read', { chatId, messageHashes });
	}
	sendReaction(chatId: ChatId, content: ChatReaction): Promise<void> {
		return invoke('send_reaction', { chatId, content });
	}
	setDetails(chatId: ChatId, details: GroupDetails): Promise<void> {
		return invoke('set_group_details', { chatId, details });
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
