import { type IChatsClient } from '../chats/chats-client';
import { ChatsStore } from '../chats/chats-store';
import { type IMessagesClient } from '../chats/messages-client';
import { ContactsStore } from '../contacts/contacts-store';
import { type IDirectChatClient } from '../direct-chats/direct-chat-client';
import { type IGroupChatClient } from '../group-chats/group-chat-client';
import { LogsStore } from '../p2panda/logs-store';
import { AgentId, TopicId } from '../p2panda/types';
import { TombstoneStore } from '../tombstones/tombstone-store';
import { Payload } from '../types';
import { type LocalStorageLogsClient } from './client';
import { MockDirectChatClient } from './direct-chat-client';
import { MockGroupChatClient } from './group-chat-client';
import { MockMessagesClient } from './messages-client';

export class MockChatsStore extends ChatsStore {
	constructor(
		logsStore: LogsStore<Payload>,
		contactsStore: ContactsStore,
		tombstoneStore: TombstoneStore,
		client: IChatsClient,
		private mockLogsClient: LocalStorageLogsClient,
		private agentId: AgentId,
		private deviceGroupTopicId: TopicId,
	) {
		super(logsStore, contactsStore, tombstoneStore, client);
	}

	protected directChatClient(): IDirectChatClient {
		return new MockDirectChatClient(this.agentId);
	}

	protected groupChatClient(): IGroupChatClient {
		return new MockGroupChatClient();
	}

	protected messagesClient(): IMessagesClient {
		return new MockMessagesClient(this.mockLogsClient, this.deviceGroupTopicId);
	}
}
