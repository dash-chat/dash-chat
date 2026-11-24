import { type IChatsClient } from '../chats/chats-client';
import { ChatsStore } from '../chats/chats-store';
import { type IMessagesClient } from '../chats/messages-client';
import { ContactsStore } from '../contacts/contacts-store';
import { type IGroupChatClient } from '../group-chats/group-chat-client';
import { MessageAckStore } from '../message-acks/message-ack-store';
import { LogsStore } from '../p2panda/logs-store';
import { TopicId } from '../p2panda/types';
import { TombstoneStore } from '../tombstones/tombstone-store';
import { Payload } from '../types';
import { type LocalStorageLogsClient } from './client';
import { MockGroupChatClient } from './group-chat-client';
import { MockMessagesClient } from './messages-client';

export class MockChatsStore extends ChatsStore {
	constructor(
		logsStore: LogsStore<Payload>,
		contactsStore: ContactsStore,
		tombstoneStore: TombstoneStore,
		messageAckStore: MessageAckStore,
		client: IChatsClient,
		private mockLogsClient: LocalStorageLogsClient,
		private deviceGroupTopicId: TopicId,
	) {
		super(logsStore, contactsStore, tombstoneStore, messageAckStore, client);
	}

	protected groupChatClient(): IGroupChatClient {
		return new MockGroupChatClient();
	}

	protected messagesClient(): IMessagesClient {
		return new MockMessagesClient(this.mockLogsClient, this.deviceGroupTopicId);
	}
}
