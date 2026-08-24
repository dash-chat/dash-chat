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
				const fetchTombstones = async () => {
					const tombstones = await this.client.getTombstones(topic);
					// Merge rather than replace: an `onNewTombstones` event can
					// arrive and merge into `state.value` before this in-flight
					// fetch (started before that tombstone was persisted)
					// resolves, and a wholesale replace would clobber it.
					state.value = { ...tombstones, ...(state.value || {}) };
				};

				fetchTombstones();
				const interval = POLLING_ENABLED
					? setInterval(fetchTombstones, POLL_INTERVAL_MS)
					: undefined;

				const unsub = this.client.onNewTombstones(topic, tombstone => {
					state.value = {
						...(state.value || {}),
						[tombstone.hash]: tombstone.reason,
					};
				});

				return () => {
					clearInterval(interval);
					unsub();
				};
			}),
	);
}
