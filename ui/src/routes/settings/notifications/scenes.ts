import type { Scene } from '$lib/scenes/scene';

export default [
	{ name: 'notifications', path: '/settings/notifications' },
	{
		name: 'notifications-turned-on',
		path: '/settings/notifications',
		steps: [{ tap: 'notifications-toggle' }],
	},
	{
		name: 'error-notifications-not-allowed',
		path: '/settings/notifications',
		steps: [
			() => {
				Object.defineProperty(window, 'Notification', {
					value: {
						permission: 'denied',
						requestPermission: async () => 'denied',
					},
				});
			},
			() => {
				document
					.querySelector<HTMLElement>(
						'[data-testid="notifications-toggle"] input',
					)
					?.click();
			},
		],
	},
] satisfies Scene[];
