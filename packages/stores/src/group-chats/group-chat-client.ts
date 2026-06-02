import { invoke } from '@tauri-apps/api/core';

import { AgentId } from '../p2panda/types';
import { ChatId, MessageContent } from '../types';

export interface GroupMemberData {
	chatMemberId: AgentId;
	isAdmin: boolean;
}

export interface IGroupChatClient {
	/// Members
	getMembers(chatId: ChatId): Promise<GroupMemberData[]>;
	addMember(chatId: ChatId, member: AgentId): Promise<void>;
	removeMember(chatId: ChatId, member: AgentId): Promise<void>;

	promoteToAdministrator(chatId: ChatId, member: AgentId): Promise<void>;
	demoteFromAdministrator(chatId: ChatId, member: AgentId): Promise<void>;

	/// Messages

	sendMessage(chatId: ChatId, content: MessageContent): Promise<void>;

	leaveGroup(): Promise<void>;
	deleteGroup(): Promise<void>;
}

export class GroupChatClient implements IGroupChatClient {
	async getMembers(chatId: ChatId): Promise<GroupMemberData[]> {
		const members: [AgentId, boolean][] = await invoke('get_group_members', {
			chatId,
		});
		return members.map(([chatMemberId, isAdmin]) => ({
			chatMemberId,
			isAdmin,
		}));
	}

	async addMember(chatId: ChatId, member: AgentId): Promise<void> {
		throw new Error('addMember not implemented');
	}
	async removeMember(chatId: ChatId, member: AgentId): Promise<void> {}

	sendMessage(topic: ChatId, content: MessageContent): Promise<void> {
		return invoke('send_message', { topic, content });
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

const sleep = (ms: number) =>
	new Promise(r => setTimeout(() => r(undefined), ms));
// // Store tied to a specific group chat
// export interface GroupChatStore {
// 	/// Info

// 	groupInfo(): AsyncSignal<GroupInfo>;

// 	/// Members

// 	members(): AsyncSignal<GroupMember[]>;

// 	addMember(userId: UserId): Promise<void>;

// 	removeMember(userId: UserId): Promise<void>;

// 	promoteToAdmin(userId: UserId): Promise<void>;

// 	demoteFromAdmin(userId: UserId): Promise<void>;

// 	/// Messages

// 	// Get all messages for this group chat
// 	messages(): AsyncSignal<Message[]>;

// 	// Sends a message in this group chat
// 	sendMessage(messageContent: MessageContent): Promise<MessageId>;

// 	/// Typing indicator

// 	// Sends a typing indicator signal
// 	sendTypingIndicatorSignal(): Promise<void>;

// 	// Receive typing indicator signal from the given user
// 	onTypingIndicatorSignal(handler: (userId: UserId) => void): UnsubscribeFn;
// }
