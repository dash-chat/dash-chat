import type { DeviceId, Hash } from '../p2panda/types';

export const MESSAGE_SET_TIMEFRAME_INTERVAL_MS = 60 * 1000; // 1 minute

export interface EventGroupsInDay<T> {
	day: Date;
	eventsGroups: Array<EventGroup<T>>;
}

export interface EventWithProvenance<T> {
	event: T;
	timestamp: number;
	/** Lamport counter of the underlying operation; `0` for events whose
	 * operations don't carry one (reports, blocks, group control, pre-lamport
	 * messages). Primary ordering criterion — see `orderEventsAscending`. */
	lamport: number;
	author: DeviceId;
	type: string;
}

export type EventGroup<T> = Array<[Hash, T]>;

export interface LamportOrderable {
	timestamp: number;
	lamport: number;
	author: DeviceId;
}

/** Events in display order: lamport first — so a reply never sorts before the
 * message it answers even when the authors' clocks disagree — with the
 * wall-clock timestamp and author as tiebreakers. Events without a lamport
 * (`0`) are anchored to the highest lamport among events timestamped at or
 * before them, which keeps them in their wall-clock position within the
 * lamport sequence instead of all sorting first. */
export function orderEventsAscending<E extends LamportOrderable>(
	events: Record<Hash, E>,
): Array<[Hash, E]> {
	const byTimestamp = Object.entries(events).sort(
		(m1, m2) =>
			m1[1].timestamp - m2[1].timestamp ||
			m1[1].author.localeCompare(m2[1].author),
	);
	let maxLamport = 0;
	const anchored = byTimestamp.map(([hash, event]) => {
		maxLamport = Math.max(maxLamport, event.lamport);
		const lamport = event.lamport > 0 ? event.lamport : maxLamport;
		return { hash, event, lamport };
	});
	anchored.sort(
		(m1, m2) =>
			m1.lamport - m2.lamport ||
			m1.event.timestamp - m2.event.timestamp ||
			m1.event.author.localeCompare(m2.event.author),
	);
	return anchored.map(({ hash, event }) => [hash, event] as [Hash, E]);
}

export function groupEventsInDays<T>(
	events: Record<Hash, EventWithProvenance<T>>,
	agentSets: Array<Array<DeviceId>>,
): Array<EventGroupsInDay<T>> {
	const eventsGroupsInDay: EventGroupsInDay<EventWithProvenance<T>>[] = [];
	const orderedAscendingEvents = orderEventsAscending(events);
	for (const [eventHash, event] of orderedAscendingEvents) {
		if (eventsGroupsInDay.length === 0) {
			const date = new Date(event.timestamp);
			date.setHours(0);
			date.setMinutes(0);
			date.setSeconds(0);
			date.setMilliseconds(0);
			eventsGroupsInDay.push({
				eventsGroups: [[[eventHash, event]]],
				day: date,
			});
		} else {
			const lastEventGroupsInDay =
				eventsGroupsInDay[eventsGroupsInDay.length - 1];
			const lastEventGroup =
				lastEventGroupsInDay.eventsGroups[
					lastEventGroupsInDay.eventsGroups.length - 1
				];

			const lastEvent = lastEventGroup[lastEventGroup.length - 1][1];

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
				Math.abs(event.timestamp - lastEvent.timestamp) <
				MESSAGE_SET_TIMEFRAME_INTERVAL_MS;
			const sameType = lastEvent.type === event.type;

			const date = new Date(event.timestamp);
			date.setHours(0);
			date.setMinutes(0);
			date.setSeconds(0);
			date.setMilliseconds(0);

			// `<=` rather than `===`: lamport order can place a skew-stamped
			// event after one with a later wall-clock day; clamping into the
			// current day keeps the day headers monotonic.
			if (date.valueOf() <= lastEventGroupsInDay.day.valueOf()) {
				if (sameProvenance && sameTimeframe && sameType) {
					lastEventGroup.push([eventHash, event]);
				} else {
					lastEventGroupsInDay.eventsGroups.push([[eventHash, event]]);
				}
			} else {
				eventsGroupsInDay.push({
					eventsGroups: [[[eventHash, event]]],
					day: date,
				});
			}
		}
	}
	const eventsGroups: EventGroupsInDay<T>[] = eventsGroupsInDay.map(
		eventGroup => ({
			day: eventGroup.day,
			eventsGroups: eventGroup.eventsGroups.map(group =>
				group.map(([hash, e]) => [hash, e.event]),
			),
		}),
	);
	return eventsGroups;
}
