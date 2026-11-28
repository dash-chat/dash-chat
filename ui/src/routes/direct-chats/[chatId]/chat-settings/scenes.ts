import { scene } from '$lib/scenes';
import { ALICE, directChatPath } from '$lib/scenes/demo-world';
import { backend, deviceGroupOperation, minutesAgo } from 'dash-chat-mock';

const settings = `${directChatPath(ALICE)}/chat-settings`;

function blockAlice(): void {
	deviceGroupOperation(
		{ type: 'BlockAgent', payload: ALICE.agentId },
		minutesAgo(1),
	);
}

scene('chat-settings', async ({ open }) => {
	await open(settings);
});

scene('block-contact-dialog', async ({ page, open }) => {
	await open(settings);
	await page.getByTestId('chat-settings-block-btn').first().click();
});

scene('report-contact-dialog', async ({ page, open }) => {
	await open(settings);
	await page.getByTestId('chat-settings-report-btn').first().click();
});

scene('chat-settings-blocked', async ({ open }) => {
	blockAlice();
	await open(settings);
});

scene('unblock-contact-dialog', async ({ page, open }) => {
	blockAlice();
	await open(settings);
	await page.getByTestId('chat-settings-unblock-btn').first().click();
});

scene('block-contact-result', async ({ page, open }) => {
	await open(settings);
	await page.getByTestId('chat-settings-block-btn').first().click();
	await page.getByTestId('block-contact-confirm').first().click();
});

scene('unblock-contact-result', async ({ page, open }) => {
	blockAlice();
	await open(settings);
	await page.getByTestId('chat-settings-unblock-btn').first().click();
	await page.getByTestId('unblock-contact-confirm').first().click();
});

scene('report-contact-result', async ({ page, open }) => {
	await open(settings);
	await page.getByTestId('chat-settings-report-btn').first().click();
	await page.getByTestId('report-contact-confirm').first().click();
});

scene('report-contact-failed', async ({ page, open }) => {
	backend.fails('report_contact');
	await open(settings);
	await page.getByTestId('chat-settings-report-btn').first().click();
	await page.getByTestId('report-contact-confirm').first().click();
});
