import { listen } from '@tauri-apps/api/event';
import { type UnsubscribeFunction } from 'emittery';

import type { SystemEvent } from '../types';

/**
 * Run `handler` for every `dashchat://system-event` of one of `types`, from
 * now on, until the returned function is called — which is honoured even
 * when it comes before the listener finished registering.
 */
export function onSystemEvent<T extends SystemEvent['type']>(
	types: T | T[],
	handler: (event: Extract<SystemEvent, { type: T }>) => void,
): UnsubscribeFunction {
	const wanted: SystemEvent['type'][] = Array.isArray(types) ? types : [types];
	let unlisten: (() => void) | undefined;
	let cancelled = false;
	listen('dashchat://system-event', e => {
		const event = e.payload as SystemEvent;
		if (wanted.includes(event.type)) {
			handler(event as Extract<SystemEvent, { type: T }>);
		}
	}).then(u => {
		if (cancelled) u();
		else unlisten = u;
	});
	return () => {
		cancelled = true;
		if (unlisten) unlisten();
	};
}
