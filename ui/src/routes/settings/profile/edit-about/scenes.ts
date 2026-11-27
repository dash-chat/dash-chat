import type { Scene } from '$lib/scenes/scene';
import { open } from '$lib/scenes/steps';

export default [
	{
		name: 'edit-about',
		setup: () => open('/settings/profile/edit-about'),
	},
] satisfies Scene[];
