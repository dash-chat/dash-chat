import { type ReactivePromise, reactive, relay } from 'signalium';

import type { TopicId } from '../p2panda/types';
import type { ITombstoneClient } from './tombstone-client';
import { pollingRequired } from '../utils/polling-required';
import { Tombstone } from '../types';

const POLL_INTERVAL_MS = 1_000;
const POLLING_ENABLED = pollingRequired();


export class TombstoneStore {
    constructor(public client: ITombstoneClient) { }
    
    tombstones = reactive((topic: TopicId): ReactivePromise<Tombstone[]> => relay(state => {
        const fetchTombstones = async () => {            
            let tombstones = await this.client.getTombstones(topic);
            if (tombstones.length > 0) {
                state.value = tombstones;
            }
        }

        fetchTombstones();
        const interval = POLLING_ENABLED
            ? setInterval(fetchTombstones, POLL_INTERVAL_MS)
            : undefined;

        const unsub = this.client.onNewTombstones(topic, (tombstone) => {
            state.value = [...(state.value || []), tombstone];
        });

        return () => {
            clearInterval(interval);
            unsub();
        }
    }))
}
