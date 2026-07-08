import { AgentId, DeviceId, Hash } from '../p2panda/types';
import { ChatId, ChatReaction, GroupInfo, OutgoingMedia } from '../types';
import { invokeAfterSetup } from '../utils/invoke-after-setup';

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
	): Promise<Hash>;
	markMessagesRead(chatId: ChatId, messageHashes: Hash[]): Promise<void>;
	editMessage(chatId: ChatId, editHash: Hash, message: string): Promise<Hash>;
	deleteMessage(chatId: ChatId, targetHash: Hash): Promise<Hash>;
	sendReaction(chatId: ChatId, content: ChatReaction): Promise<void>;

	setInfo(chatId: ChatId, info: GroupInfo): Promise<void>;

	leaveGroup(chatId: ChatId): Promise<void>;
	deleteGroup(): Promise<void>;
}

export class GroupChatClient implements IGroupChatClient {
	async getMembers(chatId: ChatId): Promise<GroupMember[]> {
		return invokeAfterSetup('get_group_members', { chatId });
	}

	async addMember(chatId: ChatId, member: AgentId): Promise<void> {
		await invokeAfterSetup('add_group_member', { chatId, agentId: member });
	}
	async removeMember(chatId: ChatId, member: AgentId): Promise<void> {
		await invokeAfterSetup('remove_group_member', { chatId, agentId: member });
	}

	sendMessage(
		chatId: ChatId,
		message: string,
		media: OutgoingMedia | null,
	): Promise<Hash> {
		return invokeAfterSetup('send_message', {
			chatId,
			message,
			media,
		});
	}
	markMessagesRead(chatId: ChatId, messageHashes: Hash[]): Promise<void> {
		return invokeAfterSetup('mark_messages_read', { chatId, messageHashes });
	}
	sendReaction(chatId: ChatId, content: ChatReaction): Promise<void> {
		return invokeAfterSetup('send_reaction', { chatId, content });
	}
	editMessage(chatId: ChatId, editHash: Hash, message: string): Promise<Hash> {
		return invokeAfterSetup('edit_message', { chatId, editHash, message });
	}
	deleteMessage(chatId: ChatId, targetHash: Hash): Promise<Hash> {
		return invokeAfterSetup('delete_message', { chatId, targetHash });
	}
	setInfo(chatId: ChatId, info: GroupInfo): Promise<void> {
		return invokeAfterSetup('set_group_info', { chatId, info });
	}
	async promoteToAdministrator(
		chatId: ChatId,
		member: AgentId,
	): Promise<void> {}
	async demoteFromAdministrator(
		chatId: ChatId,
		member: AgentId,
	): Promise<void> {}

	async leaveGroup(chatId: ChatId): Promise<void> {
		await invokeAfterSetup('leave_group', { chatId });
	}

	async deleteGroup(): Promise<void> {}
}
