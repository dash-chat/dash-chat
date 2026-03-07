import type { IDirectChatClient } from '../direct-chats/direct-chat-client';
import type { AgentId, Hash } from '../p2panda/types';
import type { ChatId, ChatReaction, MessageContent } from '../types';
import { hash, type LocalStorageLogsClient } from './client';

export class MockDirectChatClient implements IDirectChatClient {
	constructor(
		private logsClient: LocalStorageLogsClient,
		private agentId: AgentId,
	) {}

	async chatId(peer: AgentId): Promise<ChatId> {
		return hash([this.agentId, peer].sort().join(':'));
	}

	async sendMessage(chatId: ChatId, content: MessageContent): Promise<void> {
		await this.logsClient.create(chatId, {
			type: 'Chat',
			payload: { type: 'Message', payload: content },
		});
	}

	async markMessagesRead(_chatId: ChatId, _messageHashes: Hash[]): Promise<void> {}

	async sendReaction(chatId: ChatId, content: ChatReaction): Promise<void> {
		await this.logsClient.create(chatId, {
			type: 'Chat',
			payload: { type: 'Reaction', payload: content },
		});
	}
}
