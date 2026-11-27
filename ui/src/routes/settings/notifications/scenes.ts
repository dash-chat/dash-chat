import type { Scene } from '$lib/scenes';
import { open, tid } from '$lib/scenes/steps';
import { present, settled, tap } from 'i18n-screenshots/scene';

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
			const toggle = await present(`${tid('notifications-toggle')} input`);
			toggle.click();
		},
	},
] satisfies Scene[];
