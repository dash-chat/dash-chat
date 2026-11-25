import type { Scene } from '$lib/scenes/scene';
import { mockBackend } from 'dash-chat-stores';

export default [
	{ name: 'edit-name', path: '/settings/profile/edit-name' },
	{
		name: 'error-saving-profile',
		path: '/settings/profile/edit-name',
		seed() {
			mockBackend.failingCommands = ['set_profile:AuthorOperation'];
		},
		steps: [
			{ type: 'Robin', into: 'edit-name-name' },
			{ tap: 'edit-name-save-btn' },
		],
	},
] satisfies Scene[];
