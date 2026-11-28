import { scene } from '$lib/scenes';
import { onlyMyProfile } from '$lib/scenes/demo-world';

scene('new-group-members', async ({ open }) => {
	await open('/new-group');
});

scene('new-group-info', async ({ page, open }) => {
	await open('/new-group');
	await page.getByTestId('selectable-contact-item').first().click();
	await page.getByTestId('new-group-next').first().click();
});

scene(
	'ios-new-group-members',
	async ({ open }) => {
		await open('/new-group');
	},
	{ device: 'iphone' },
);

scene('new-group-no-contacts', async ({ open }) => {
	onlyMyProfile();
	await open('/new-group');
});

scene('new-group-member-selected', async ({ page, open }) => {
	await open('/new-group');
	await page.getByTestId('selectable-contact-item').first().click();
});

scene('new-group-info-without-members', async ({ page, open }) => {
	await open('/new-group');
	await page.getByTestId('new-group-next').first().click();
});
