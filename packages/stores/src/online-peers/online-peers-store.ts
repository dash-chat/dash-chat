import { type ReactivePromise, reactive } from 'signalium';
import { subscribe } from 'tauri-plugin-subscriptions';

import { ContactsStore } from '../contacts/contacts-store';
import type { AgentId, DeviceId, TopicId } from '../p2panda/types';

export interface IOnlinePeersStore {
	onlinePeers(topicId: TopicId): ReactivePromise<AgentId[]>;
}

/** A peer is online on a topic while one of its devices keeps up presence there:
 * what we publish on it reaches them right away, directly or through another
 * member. */
export class OnlinePeersStore implements IOnlinePeersStore {
	constructor(protected contactsStore: ContactsStore) {}

	onlineDevices = reactive((topicId: TopicId) =>
		subscribe<DeviceId[]>('subscribe_to_online_peers', { topicId }),
	);

	onlinePeers = reactive(async (topicId: TopicId) => {
		const devices = await this.onlineDevices(topicId);
		const agents = await this.contactsStore.agentsForDevices(new Set(devices));
		const myAgentId = await this.contactsStore.myAgentId();
		return Array.from(new Set(Object.values(agents))).filter(
			agent => agent !== myAgentId,
		);
	});
}
