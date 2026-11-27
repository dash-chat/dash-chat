import { TINY_PNG_BYTES } from '$lib/scenes/demo-world';
import type { Scene } from '$lib/scenes/scene';
import { open } from '$lib/scenes/steps';
import { settled, tap } from 'i18n-screenshots/scene';

export default [
	{
		name: 'edit-photo',
		setup: () => open('/settings/profile/edit-photo'),
	},
	{
		name: 'edit-photo-text',
		async setup() {
			await open('/settings/profile/edit-photo');
			await tap('edit-photo-text');
		},
	},
	{
		name: 'edit-photo-chosen',
		async setup() {
			await open('/settings/profile/edit-photo');
			window.__test.interceptFilePickers([
				{ name: 'me.png', mimeType: 'image/png', bytes: TINY_PNG_BYTES },
			]);
			await settled();
			await tap('edit-photo-gallery');
		},
	},
] satisfies Scene[];
