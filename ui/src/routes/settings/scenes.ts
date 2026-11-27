import type { Scene } from '$lib/scenes/scene';
import { open } from '$lib/scenes/steps';

export default [
	{
		name: 'settings',
		setup: () => open('/settings'),
	},
] satisfies Scene[];
