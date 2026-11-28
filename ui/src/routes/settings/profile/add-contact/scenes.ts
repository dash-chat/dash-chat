import { scene } from '$lib/scenes';

scene('profile-add-contact', async ({ open }) => {
	await open('/settings/profile/add-contact');
});
