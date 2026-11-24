import type { Agent } from '../../../setup/setup-agents';
import { isMobile } from '../../../setup/test-env';
import { AndroidNotifications } from './android';
import { IosNotifications } from './ios';
import type { NotificationHelper } from './notification-helper';

export type {
	DeliveredNotification,
	NotificationHelper,
} from './notification-helper';

/** The helper for an agent whose notifications a run can read, or null for a
 * desktop, which posts none this way. */
export function readableNotificationsFor(
	agent: Agent,
): NotificationHelper | null {
	return isMobile(agent.platform) ? notificationHelperFor(agent) : null;
}

export function notificationHelperFor(agent: Agent): NotificationHelper {
	switch (agent.platform) {
		case 'ios':
			return new IosNotifications(agent);
		case 'android':
		case 'android-emulator':
			return new AndroidNotifications(agent);
		default:
			throw new Error(
				`No notification helper for platform '${agent.platform}' — push specs need an iOS or Android device`,
			);
	}
}
