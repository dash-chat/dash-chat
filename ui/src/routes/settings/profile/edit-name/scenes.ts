import type { Scene } from '$lib/scenes/scene';
import { open } from '$lib/scenes/steps';
import { setMockBackend } from 'dash-chat-mock';
import { fill, tap } from 'i18n-screenshots/scene';

export default [
	{
		name: 'edit-name',
		setup: () => open('/settings/profile/edit-name'),
	},
	{
		name: 'error-saving-profile',
		async setup() {
			setMockBackend({ failingCommands: ['set_profile:AuthorOperation'] });
			await open('/settings/profile/edit-name');
			await fill('edit-name-name', 'Robin');
			await tap('edit-name-save-btn');
		},
	},
] satisfies Scene[];
