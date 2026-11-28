import { field, scene } from '$lib/scenes';
import { backend } from 'dash-chat-mock';

scene('edit-name', async ({ open }) => {
	await open('/settings/profile/edit-name');
});

scene('error-saving-profile', async ({ page, open }) => {
	backend.fails('set_profile', 'AuthorOperation');
	await open('/settings/profile/edit-name');
	await field(page, 'edit-name-name').fill('Robin');
	await page.getByTestId('edit-name-save-btn').first().click();
});
