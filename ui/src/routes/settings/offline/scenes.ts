import { scene } from '$lib/scenes';
import { backend } from 'dash-chat-mock';

scene('offline', async ({ open }) => {
	await open('/settings/offline');
});

scene(
	'desktop-offline',
	async ({ open }) => {
		await open('/settings/offline');
	},
	{ device: 'desktop' },
);

scene(
	'ios-settings',
	async ({ open }) => {
		await open('/settings');
	},
	{ device: 'iphone' },
);

scene(
	'desktop-settings',
	async ({ open }) => {
		await open('/settings');
	},
	{ device: 'desktop' },
);

scene(
	'desktop-offline-hub-blocked',
	async ({ open }) => {
		backend.setSettings({ local_mailbox_enabled: true });
		backend.setWorld({ localHubBlocked: true });
		await open('/settings/offline');
	},
	{ device: 'desktop' },
);

scene(
	'linux-desktop-offline-hub-blocked',
	async ({ open }) => {
		backend.setSettings({ local_mailbox_enabled: true });
		backend.setWorld({ localHubBlocked: true });
		await open('/settings/offline');
	},
	{ device: 'linux-desktop' },
);
