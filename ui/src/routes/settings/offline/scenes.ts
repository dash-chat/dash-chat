import type { Scene } from '$lib/scenes/scene';
import { mockBackend } from 'dash-chat-stores';

export default [
	{ name: 'offline', path: '/settings/offline' },
	{ name: 'desktop-offline', path: '/settings/offline', device: 'desktop' },
	{ name: 'ios-settings', path: '/settings', device: 'iphone' },
	{ name: 'desktop-settings', path: '/settings', device: 'desktop' },
	{
		name: 'desktop-offline-hub-blocked',
		path: '/settings/offline',
		device: 'desktop',
		seed() {
			mockBackend.localHubBlocked = true;
		},
	},
	{
		name: 'linux-desktop-offline-hub-blocked',
		path: '/settings/offline',
		device: 'linux-desktop',
		seed() {
			mockBackend.localHubBlocked = true;
		},
	},
] satisfies Scene[];
