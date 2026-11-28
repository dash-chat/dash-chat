import { scene, untilAToastShows } from '$lib/scenes';

scene('notifications', async ({ open }) => {
	await open('/settings/notifications');
});

scene('notifications-turned-on', async ({ page, open }) => {
	await open('/settings/notifications');
	await page.getByTestId('notifications-toggle').locator('label').click();
});

scene('error-notifications-not-allowed', async ({ page, open }) => {
	await open('/settings/notifications');
	await page.evaluate(() =>
		Object.defineProperty(window, 'Notification', {
			value: {
				permission: 'denied',
				requestPermission: async () => 'denied',
			},
		}),
	);
	await page.getByTestId('notifications-toggle').locator('label').click();
	await untilAToastShows(page);
});
