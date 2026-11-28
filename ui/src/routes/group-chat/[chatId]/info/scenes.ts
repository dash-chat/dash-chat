import { scene } from '$lib/scenes';
import {
	ALICE,
	BOB,
	CAROL,
	DAVE,
	GROUP_CHAT_PATH,
	ME,
	describeGroup,
	groupIAdminister,
} from '$lib/scenes/demo-world';
import {
	MOCK_MAILBOX_DOWN,
	minutesAgo,
	mockBackend,
	setMockBackend,
} from 'dash-chat-mock';

const info = `${GROUP_CHAT_PATH}/info`;

scene('group-info', async ({ open }) => {
	groupIAdminister();
	await open(info);
});

scene('group-member-actions', async ({ page, open }) => {
	groupIAdminister();
	await open(info);
	await page.getByTestId('group-info-member-Alice').first().click();
});

scene('group-make-admin-dialog', async ({ page, open }) => {
	groupIAdminister();
	await open(info);
	await page.getByTestId('group-info-member-Alice').first().click();
	await page.getByTestId('group-info-make-admin').first().click();
});

scene('group-remove-member-dialog', async ({ page, open }) => {
	groupIAdminister();
	await open(info);
	await page.getByTestId('group-info-member-Alice').first().click();
	await page.getByTestId('group-info-remove-member').first().click();
});

scene('leave-group-dialog', async ({ page, open }) => {
	groupIAdminister();
	await open(info);
	await page.getByTestId('group-info-leave').first().click();
});

scene('group-edit', async ({ open }) => {
	groupIAdminister();
	await open(`${info}/edit`);
});

scene('group-add-members', async ({ open }) => {
	groupIAdminister();
	await open(`${info}/add-members`);
});

scene('home-with-group', () => {
	groupIAdminister();
});

scene('error-leaving-group-as-only-admin', async ({ page, open }) => {
	groupIAdminister();
	setMockBackend({ failingCommands: ['leave_group:LastAdmin'] });
	await open(info);
	await page.getByTestId('group-info-leave').first().click();
	await page.getByTestId('group-info-leave-confirm').first().click();
});

scene('error-leaving-group', async ({ page, open }) => {
	groupIAdminister();
	setMockBackend({ failingCommands: ['leave_group'] });
	await open(info);
	await page.getByTestId('group-info-leave').first().click();
	await page.getByTestId('group-info-leave-confirm').first().click();
});

scene('group-info-peers-online', async ({ open }) => {
	groupIAdminister();
	setMockBackend({
		mailboxes: {
			...mockBackend().mailboxes,
			cloudMailbox: MOCK_MAILBOX_DOWN,
		},
		onlinePeers: [ALICE.agentId],
	});
	await open(info);
});

scene('group-add-members-none-left', async ({ open }) => {
	groupIAdminister();
	describeGroup(
		'Hiking club',
		[ME, ALICE, BOB, CAROL, DAVE],
		[ME],
		minutesAgo(1),
	);
	await open(`${info}/add-members`);
});
