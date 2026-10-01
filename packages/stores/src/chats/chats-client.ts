import { type UnsubscribeFunction } from 'emittery';

import { Hash, VerifyingKey } from '../p2panda/types';
import { ChatId } from '../types';
import { invokeAfterSetup } from '../utils/invoke-after-setup';
import { onSystemEvent } from '../utils/system-events';

export interface IChatsClient {
	createGroup(initialMembers: VerifyingKey[]): Promise<ChatId>;
	getGroupChats(): Promise<Array<ChatId>>;
	markMessagesRead(chatId: ChatId, messageHashes: Hash[]): Promise<void>;
	/** A group chat was added. */
	onGroupChatAdded(handler: (chatId: ChatId) => void): UnsubscribeFunction;
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

	onGroupChatAdded(handler: (chatId: ChatId) => void): UnsubscribeFunction {
		return onSystemEvent('GroupChatAdded', event =>
			handler(event.payload.chat_id),
		);
	}
}
