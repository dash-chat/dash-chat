import { scene } from '$lib/scenes';

scene('developer', async ({ open }) => {
	await open('/settings/developer');
});
