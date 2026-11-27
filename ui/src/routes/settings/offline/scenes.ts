import type { Scene } from '$lib/scenes/scene';
import { open } from '$lib/scenes/steps';
import { setMockBackend } from 'dash-chat-mock';

export default [
	{
		name: 'offline',
		setup: () => open('/settings/offline'),
	},
	{
		name: 'desktop-offline',
		device: 'desktop',
		setup: () => open('/settings/offline'),
	},
	{
		name: 'ios-settings',
		device: 'iphone',
		setup: () => open('/settings'),
	},
	{
		name: 'desktop-settings',
		device: 'desktop',
		setup: () => open('/settings'),
	},
	{
		name: 'desktop-offline-hub-blocked',
		device: 'desktop',
		async setup() {
			setMockBackend({ localHubBlocked: true });
			await open('/settings/offline');
		},
	},
	{
		name: 'linux-desktop-offline-hub-blocked',
		device: 'linux-desktop',
		async setup() {
			setMockBackend({ localHubBlocked: true });
			await open('/settings/offline');
		},
	},
] satisfies Scene[];
