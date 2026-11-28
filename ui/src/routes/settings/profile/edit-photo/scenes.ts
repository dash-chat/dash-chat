import { scene } from '$lib/scenes';
import { TINY_PNG_BYTES } from '$lib/scenes/demo-world';

scene('edit-photo', async ({ open }) => {
	await open('/settings/profile/edit-photo');
});

scene('edit-photo-text', async ({ page, open }) => {
	await open('/settings/profile/edit-photo');
	await page.getByTestId('edit-photo-text').first().click();
});

scene('edit-photo-chosen', async ({ page, open }) => {
	await open('/settings/profile/edit-photo');
	await page.evaluate(
		picked => window.__test.interceptFilePickers(picked),
		[{ name: 'me.png', mimeType: 'image/png', bytes: TINY_PNG_BYTES }],
	);
	await page.getByTestId('edit-photo-gallery').first().click();
});
