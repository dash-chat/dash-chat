import type { Scene } from '$lib/scenes/scene';
import { open } from '$lib/scenes/steps';
import { settled, tap } from 'translation-screenshots/scene';

export default [
	{
		name: 'notifications',
		setup: () => open('/settings/notifications'),
	},
	{
		name: 'notifications-turned-on',
		async setup() {
			await open('/settings/notifications');
			await tap('notifications-toggle');
		},
	},
	{
		name: 'error-notifications-not-allowed',
		async setup() {
			await open('/settings/notifications');
			Object.defineProperty(window, 'Notification', {
				value: {
					permission: 'denied',
					requestPermission: async () => 'denied',
				},
			});
			await settled();
			document
				.querySelector<HTMLElement>(
					'[data-testid="notifications-toggle"] input',
				)
				?.click();
		},
	},
] satisfies Scene[];
