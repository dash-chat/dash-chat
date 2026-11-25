import { type ReactivePromise, reactive, relay } from 'signalium';

import type { IOnlinePeersStore } from '../online-peers/online-peers-store';
import type { AgentId, TopicId } from '../p2panda/types';
import { mockBackend } from './backend-state';

export class MockOnlinePeersStore implements IOnlinePeersStore {
	onlinePeers = reactive(
		(_topicId: TopicId): ReactivePromise<AgentId[]> =>
			relay<AgentId[]>(state => {
				state.value = mockBackend.onlinePeers;
				return () => {};
			}),
	);

	onlinePeersInAnyChat = reactive(
		(): ReactivePromise<AgentId[]> =>
			relay<AgentId[]>(state => {
				state.value = mockBackend.onlinePeers;
				return () => {};
			}),
	);
}
