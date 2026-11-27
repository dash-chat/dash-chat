import type { Scene } from '$lib/scenes/scene';
import { open } from '$lib/scenes/steps';

export default [
	{
		name: 'profile',
		setup: () => open('/settings/profile'),
	},
] satisfies Scene[];
