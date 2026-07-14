import { AgentId } from '../p2panda/types';
import { ChatId } from '../types';
import { invokeAfterSetup } from '../utils/invoke-after-setup';

export interface IDirectChatClient {
	chatId(peer: AgentId): Promise<ChatId>;
}

export class DirectChatClient implements IDirectChatClient {
	chatId(peer: AgentId): Promise<ChatId> {
		return invokeAfterSetup('direct_chat_id', {
			peer,
		});
	}
}
