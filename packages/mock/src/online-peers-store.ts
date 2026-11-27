import type { AgentId, IOnlinePeersStore, TopicId } from 'dash-chat-stores';
import { type ReactivePromise, reactive } from 'signalium';

import { followingTheMockBackend, mockBackend } from './backend-state';

export class MockOnlinePeersStore implements IOnlinePeersStore {
	onlinePeers = reactive(
		(_topicId: TopicId): ReactivePromise<AgentId[]> =>
			followingTheMockBackend(() => mockBackend.onlinePeers),
	);

	onlinePeersInAnyChat = reactive(
		(): ReactivePromise<AgentId[]> =>
			followingTheMockBackend(() => mockBackend.onlinePeers),
	);
}
