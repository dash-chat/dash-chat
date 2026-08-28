import type { DeviceId, Hash } from '../p2panda/types';
import type { MessageDeliveryStatus } from '../types';

export const MESSAGE_SET_TIMEFRAME_INTERVAL_MS = 60 * 1000; // 1 minute

export interface EventGroupsInDay<T> {
	day: Date;
	eventsGroups: Array<EventGroup<T>>;
}

export interface EventWithProvenance<T> {
	event: T;
	timestamp: number;
	author: DeviceId;
	type: string;
	/** Set on the viewer's own messages. Events whose statuses differ are kept
	 * in separate groups so every status transition shows its indicator, and
	 * regroup once their statuses converge again. */
	deliveryStatus?: MessageDeliveryStatus;
}

export type EventGroup<T> = Array<[Hash, T]>;

export function groupEventsInDays<T>(
	events: Record<Hash, EventWithProvenance<T>>,
	agentSets: Array<Array<DeviceId>>,
): Array<EventGroupsInDay<T>> {
	const eventsGroupsInDay: EventGroupsInDay<EventWithProvenance<T>>[] = [];
	const orderedAscendingEvents = Object.entries(events).sort(
		(m1, m2) => m1[1].timestamp - m2[1].timestamp,
	);
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
				event.timestamp - lastEvent.timestamp <
				MESSAGE_SET_TIMEFRAME_INTERVAL_MS;
			const sameType = lastEvent.type === event.type;
			const sameDeliveryStatus =
				lastEvent.deliveryStatus === event.deliveryStatus;

			const date = new Date(event.timestamp);
			date.setHours(0);
			date.setMinutes(0);
			date.setSeconds(0);
			date.setMilliseconds(0);

			if (date.valueOf() === lastEventGroupsInDay.day.valueOf()) {
				if (sameProvenance && sameTimeframe && sameType && sameDeliveryStatus) {
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
