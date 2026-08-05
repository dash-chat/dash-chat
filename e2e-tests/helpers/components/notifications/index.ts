import type { Agent } from '../../../setup/setup-agents';
import { AndroidNotifications } from './android';
import { IosNotifications } from './ios';
import type { NotificationHelper } from './notification-helper';

export type { NotificationHelper } from './notification-helper';

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
