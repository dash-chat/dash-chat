import type { Scene } from '$lib/scenes';
import { open } from '$lib/scenes/steps';

export default [
	{
		name: 'profile-add-contact',
		setup: () => open('/settings/profile/add-contact'),
	},
] satisfies Scene[];
