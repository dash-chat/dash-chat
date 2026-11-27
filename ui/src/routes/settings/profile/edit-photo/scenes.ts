import type { Scene } from '$lib/scenes/scene';
import { open } from '$lib/scenes/steps';
import { settled, tap } from 'translation-screenshots/scene';

const TINY_PNG = [
	137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0,
	0, 0, 1, 8, 6, 0, 0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 120,
	156, 99, 248, 207, 192, 240, 31, 0, 5, 0, 1, 255, 137, 153, 61, 29, 0, 0, 0,
	0, 73, 69, 78, 68, 174, 66, 96, 130,
];

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
				{ name: 'me.png', mimeType: 'image/png', bytes: TINY_PNG },
			]);
			await settled();
			await tap('edit-photo-gallery');
		},
	},
] satisfies Scene[];
