import type { IDirectChatClient } from '../direct-chats/direct-chat-client';
import type { AgentId } from '../p2panda/types';
import type { ChatId } from '../types';
import { hash } from './client';

export class MockDirectChatClient implements IDirectChatClient {
	constructor(private agentId: AgentId) {}

	async chatId(peer: AgentId): Promise<ChatId> {
		return hash([this.agentId, peer].sort().join(':'));
	}
}
