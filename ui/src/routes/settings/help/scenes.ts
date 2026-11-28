import { scene } from '$lib/scenes';

scene('help', async ({ open }) => {
	await open('/settings/help');
});

scene('help-developer-mode-enabled', async ({ page, open }) => {
	await open('/settings/help');
	for (let tap = 0; tap < 7; tap++) {
		await page.getByTestId('help-version').click();
	}
});
