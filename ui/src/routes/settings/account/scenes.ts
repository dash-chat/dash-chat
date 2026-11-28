import { scene, untilAToastShows } from '$lib/scenes';
import { setMockBackend } from 'dash-chat-mock';

scene('account', async ({ open }) => {
	await open('/settings/account');
});

scene('delete-account-dialog', async ({ page, open }) => {
	await open('/settings/account');
	await page.getByTestId('account-delete').first().click();
});

scene('error-deleting-account', async ({ page, open }) => {
	setMockBackend({ failingCommands: ['delete_account'] });
	await open('/settings/account');
	await page.getByTestId('account-delete').first().click();
	await page.getByTestId('account-delete-confirm').first().click();
	await untilAToastShows(page);
});
