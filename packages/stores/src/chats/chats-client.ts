import { Hash, VerifyingKey } from '../p2panda/types';
import { ChatId } from '../types';
import { invokeAfterSetup } from '../utils/invoke-after-setup';

export interface IChatsClient {
	createGroup(initialMembers: VerifyingKey[]): Promise<ChatId>;
	getGroupChats(): Promise<Array<ChatId>>;
	markMessagesRead(chatId: ChatId, messageHashes: Hash[]): Promise<void>;
}

export class ChatsClient implements IChatsClient {
	createGroup(initialMembers: VerifyingKey[]): Promise<ChatId> {
		return invokeAfterSetup('create_group', {
			initialMembers,
		});
	}

	getGroupChats(): Promise<Array<ChatId>> {
		return invokeAfterSetup('get_group_chats');
	}

	markMessagesRead(chatId: ChatId, messageHashes: Hash[]): Promise<void> {
		return invokeAfterSetup('mark_messages_read', {
			chatId,
			messageHashes,
		});
	}
}
