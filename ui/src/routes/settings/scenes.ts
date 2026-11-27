import type { Scene } from '$lib/scenes';
import { open } from '$lib/scenes/steps';

export default [
	{
		name: 'settings',
		setup: () => open('/settings'),
	},
] satisfies Scene[];
