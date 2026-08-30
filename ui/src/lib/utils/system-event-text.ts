import { m } from '$lib/paraglide/messages.js';
import type { BlockEvent, GroupControlEvent } from 'dash-chat-stores';

import { groupEventText } from './group-event-text';

export type SystemEvent = GroupControlEvent | BlockEvent;

export function systemEventText(event: SystemEvent): string {
	switch (event.kind) {
		case 'contact_blocked':
			return m.youBlockedContact({ name: event.contactName || m.someone() });
		case 'contact_unblocked':
			return m.youUnblockedContact({ name: event.contactName || m.someone() });
		default:
			return groupEventText(event);
	}
}
