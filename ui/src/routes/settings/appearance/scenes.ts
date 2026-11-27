import type { Scene } from '$lib/scenes';
import { open } from '$lib/scenes/steps';

export default [
	{
		name: 'appearance',
		setup: () => open('/settings/appearance'),
	},
] satisfies Scene[];
