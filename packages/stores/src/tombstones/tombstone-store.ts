import { type ReactivePromise, reactive, relay } from 'signalium';

import type { TopicId } from '../p2panda/types';
import { Tombstones } from '../types';
import { pollingRequired } from '../utils/polling-required';
import type { ITombstoneClient } from './tombstone-client';

const POLL_INTERVAL_MS = 1_000;
const POLLING_ENABLED = pollingRequired();

export class TombstoneStore {
	constructor(public client: ITombstoneClient) {}

	tombstones = reactive(
		(topic: TopicId): ReactivePromise<Tombstones> =>
			relay(state => {
				state.value = {};
				const fetchTombstones = async () => {
					const tombstones = await this.client.getTombstones(topic);
					state.value = tombstones;
				};

				fetchTombstones();
				const interval = POLLING_ENABLED
					? setInterval(fetchTombstones, POLL_INTERVAL_MS)
					: undefined;

				const unsub = this.client.onNewTombstones(topic, tombstone => {
					state.value = { ...(state.value || {}), [tombstone.hash]: tombstone.reason };
				});

				return () => {
					clearInterval(interval);
					unsub();
				};
			}),
	);
}
