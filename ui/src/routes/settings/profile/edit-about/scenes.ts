import type { Scene } from '$lib/scenes';
import { open } from '$lib/scenes/steps';

export default [
	{
		name: 'edit-about',
		setup: () => open('/settings/profile/edit-about'),
	},
] satisfies Scene[];
