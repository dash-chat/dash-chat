import { scene } from '$lib/scenes';

scene('profile', async ({ open }) => {
	await open('/settings/profile');
});
