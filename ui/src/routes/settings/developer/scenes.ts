import type { Scene } from '$lib/scenes/scene';
import { open } from '$lib/scenes/steps';

export default [
	{
		name: 'developer',
		setup: () => open('/settings/developer'),
	},
] satisfies Scene[];
