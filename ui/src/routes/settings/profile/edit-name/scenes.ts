import { field, scene } from '$lib/scenes';
import { setMockBackend } from 'dash-chat-mock';

scene('edit-name', async ({ open }) => {
	await open('/settings/profile/edit-name');
});

scene('error-saving-profile', async ({ page, open }) => {
	setMockBackend({ failingCommands: ['set_profile:AuthorOperation'] });
	await open('/settings/profile/edit-name');
	await field(page, 'edit-name-name').fill('Robin');
	await page.getByTestId('edit-name-save-btn').first().click();
});
