import { field, scene } from '$lib/scenes';
import { TINY_PNG_BYTES, reportingIsUnavailable } from '$lib/scenes/demo-world';
import { backend } from 'dash-chat-mock';

const contactUs = '/settings/help/contact-us';

type Stage = Parameters<NonNullable<Parameters<typeof scene>[1]>>[0];

/** Open the form and pick `files` as the screenshots to send with it. */
async function pickAsScreenshots(
	{ page, open }: Stage,
	files: Parameters<typeof window.__test.interceptFilePickers>[0],
): Promise<void> {
	await open(contactUs);
	await page.evaluate(
		picked => window.__test.interceptFilePickers(picked),
		files,
	);
	await page.getByTestId('contact-us-screenshots-add').first().click();
}

async function sendFeedback({ page, open }: Stage): Promise<void> {
	await open(contactUs);
	await page
		.getByTestId('contact-us-reason-select')
		.locator('select')
		.selectOption({ index: 1 });
	await field(page, 'contact-us-message-input').fill('The app is great');
	await page.getByTestId('contact-us-send-btn').first().click();
}

scene('contact-us', async ({ open }) => {
	await open(contactUs);
});

scene('contact-us-with-screenshots', async stage => {
	await pickAsScreenshots(stage, [
		{ name: 'crash.png', mimeType: 'image/png', bytes: TINY_PNG_BYTES },
	]);
});

scene('error-feedback-unsupported-image', async stage => {
	await pickAsScreenshots(stage, [
		{ name: 'notes.txt', mimeType: 'text/plain', bytes: [104, 105] },
	]);
});

scene('error-feedback-screenshot-too-large', async stage => {
	await pickAsScreenshots(stage, [
		{ name: 'big.gif', mimeType: 'image/gif', size: 9 * 1024 * 1024 },
	]);
});

scene('error-feedback-too-many-screenshots', async stage => {
	await pickAsScreenshots(
		stage,
		['a', 'b', 'c', 'd', 'e', 'f'].map(name => ({
			name: `${name}.png`,
			mimeType: 'image/png',
			bytes: TINY_PNG_BYTES,
		})),
	);
});

scene('contact-us-sent', sendFeedback);

scene('contact-us-queued', async stage => {
	backend.answers('plugin:sentry-reporting|send_feedback', 'queued');
	await sendFeedback(stage);
});

scene('contact-us-unavailable', async ({ page, open }) => {
	await reportingIsUnavailable(page);
	await open(contactUs);
});
