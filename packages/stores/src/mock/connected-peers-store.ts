import { type ReactivePromise, reactive, relay } from 'signalium';

import type { IConnectedPeersStore } from '../connected-peers/connected-peers-store';
import type { AgentId, TopicId } from '../p2panda/types';

export class MockConnectedPeersStore implements IConnectedPeersStore {
	connectedPeers = reactive(
		(_topicId: TopicId): ReactivePromise<AgentId[]> =>
			relay<AgentId[]>(state => {
				state.value = [];
				return () => {};
			}),
	);
}
