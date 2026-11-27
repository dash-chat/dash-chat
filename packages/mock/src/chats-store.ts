import {
	ChatsStore,
	ContactsStore,
	type IChatsClient,
	type IGroupChatClient,
	type IMessagesClient,
	LogsStore,
	MessageAckStore,
	Payload,
	TombstoneStore,
	TopicId,
} from 'dash-chat-stores';

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
