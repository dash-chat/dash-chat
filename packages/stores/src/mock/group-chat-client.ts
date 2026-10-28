import type {
	GroupMember,
	IGroupChatClient,
} from '../group-chats/group-chat-client';
import type { SimplifiedOperation } from '../p2panda/simplified-types';
import type { AgentId } from '../p2panda/types';
import type { ChatId, GroupInfo, Payload } from '../types';

export class MockGroupChatClient implements IGroupChatClient {
	async getMembers(_chatId: ChatId): Promise<GroupMember[]> {
		return [];
	}
	async getControlOps(
		_chatId: ChatId,
	): Promise<SimplifiedOperation<Payload>[]> {
		return [];
	}
	onGroupMembersChanged(): () => void {
		return () => {};
	}
	async addMember(_chatId: ChatId, _member: AgentId): Promise<void> {}
	async removeMember(_chatId: ChatId, _member: AgentId): Promise<void> {}
	async promoteToAdministrator(
		_chatId: ChatId,
		_member: AgentId,
	): Promise<void> {}
	async demoteFromAdministrator(
		_chatId: ChatId,
		_member: AgentId,
	): Promise<void> {}
	async setInfo(_chatId: ChatId, _info: GroupInfo): Promise<void> {}
	async leaveGroup(): Promise<void> {}
	async deleteGroup(): Promise<void> {}
}
