import { field, scene } from '$lib/scenes';

scene('new-message', async ({ open }) => {
	await open('/new-message');
});

scene('new-message-no-match', async ({ page, open }) => {
	await open('/new-message');
	await field(page, 'new-message-search').fill('zzzz');
});

scene(
	'desktop-new-message-contact-menu',
	async ({ page, open }) => {
		await open('/new-message');
		await page.getByTestId('new-message-contact-item').first().hover();
	},
	{ device: 'desktop' },
);
