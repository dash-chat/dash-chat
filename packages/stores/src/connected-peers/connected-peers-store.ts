import { type ReactivePromise, reactive } from 'signalium';
import { subscribe } from 'tauri-plugin-subscriptions';

import { ContactsStore } from '../contacts/contacts-store';
import type { AgentId, DeviceId, TopicId } from '../p2panda/types';

export interface IConnectedPeersStore {
	connectedPeers(topicId: TopicId): ReactivePromise<AgentId[]>;
}

/** A peer is connected on a topic while one of its devices has an open p2p sync
 * session with us there: what we publish on it reaches them right away. That
 * is a transport fact, not a membership one: sessions aren't checked against
 * who is in the chat. */
export class ConnectedPeersStore implements IConnectedPeersStore {
	constructor(protected contactsStore: ContactsStore) {}

	connectedDevices = reactive((topicId: TopicId) =>
		subscribe<DeviceId[]>('subscribe_to_connected_peers', { topicId }),
	);

	connectedPeers = reactive(async (topicId: TopicId) => {
		const devices = await this.connectedDevices(topicId);
		const agents = await this.contactsStore.agentsForDevices(new Set(devices));
		const myAgentId = await this.contactsStore.myAgentId();
		return Array.from(new Set(Object.values(agents))).filter(
			agent => agent !== myAgentId,
		);
	});
}
