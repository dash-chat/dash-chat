import { scene } from '$lib/scenes';

scene('edit-about', async ({ open }) => {
	await open('/settings/profile/edit-about');
});
