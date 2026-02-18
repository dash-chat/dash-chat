import { DeviceId, Hash } from '../p2panda/types';

export const MESSAGE_SET_TIMEFRAME_INTERVAL_MS = 60 * 1000; // 1 minute

export interface EventSetsInDay<T> {
	day: Date;
	eventsSets: Array<EventSet<T>>;
}

export interface EventWithProvenance<T> {
	event: T;
	timestamp: number;
	author: DeviceId;
	type: string;
}

export type EventSet<T> = Array<[Hash, T]>;

export function orderInEventSets<T>(
	events: Record<Hash, EventWithProvenance<T>>,
	agentSets: Array<Array<DeviceId>>,
): Array<EventSetsInDay<T>> {
	const eventsSetsInDay: EventSetsInDay<EventWithProvenance<T>>[] = [];
	const orderedAscendingEvents = Object.entries(events).sort(
		(m1, m2) => m1[1].timestamp - m2[1].timestamp,
	);
	for (const [eventHash, event] of orderedAscendingEvents) {
		if (eventsSetsInDay.length === 0) {
			const date = new Date(event.timestamp);
			date.setHours(0);
			date.setMinutes(0);
			date.setSeconds(0);
			date.setMilliseconds(0);
			eventsSetsInDay.push({
				eventsSets: [[[eventHash, event]]],
				day: date,
			});
		} else {
			const lastEventSetsInDay = eventsSetsInDay[eventsSetsInDay.length - 1];
			const lastEventSet =
				lastEventSetsInDay.eventsSets[lastEventSetsInDay.eventsSets.length - 1];

			const lastEvent = lastEventSet[lastEventSet.length - 1][1];

			const lastMessageAgentSet = agentSets.find(agents =>
				agents.some(agent => agent === lastEvent.author),
			);

			const currentMessageAgentSet = agentSets.find(agents =>
				agents.some(agent => agent === event.author),
			);

			const sameProvenance =
				lastMessageAgentSet !== undefined &&
				currentMessageAgentSet !== undefined &&
				lastMessageAgentSet === currentMessageAgentSet;
			const sameTimeframe =
				event.timestamp - lastEvent.timestamp <
				MESSAGE_SET_TIMEFRAME_INTERVAL_MS;
			const sameType = lastEvent.type === event.type;

			const date = new Date(event.timestamp);
			date.setHours(0);
			date.setMinutes(0);
			date.setSeconds(0);
			date.setMilliseconds(0);

			if (date.valueOf() === lastEventSetsInDay.day.valueOf()) {
				if (sameProvenance && sameTimeframe && sameType) {
					lastEventSet.push([eventHash, event]);
				} else {
					lastEventSetsInDay.eventsSets.push([[eventHash, event]]);
				}
			} else {
				eventsSetsInDay.push({
					eventsSets: [[[eventHash, event]]],
					day: date,
				});
			}
		}
	}
	const eventsSets: EventSetsInDay<T>[] = eventsSetsInDay.map(eventSet => ({
		day: eventSet.day,
		eventsSets: eventSet.eventsSets.map(set =>
			set.map(([hash, e]) => [hash, e.event]),
		),
	}));
	return eventsSets;
}
