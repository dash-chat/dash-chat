import type { Scene } from '$lib/scenes';
import { TINY_PNG_BYTES, reportingIsUnavailable } from '$lib/scenes/demo-world';
import { open, tid } from '$lib/scenes/steps';
import { setMockBackend } from 'dash-chat-mock';
import { fill, present, settled, tap } from 'i18n-screenshots/scene';

async function chooseAReason(): Promise<void> {
	const reason = await present<HTMLSelectElement>(
		`${tid('contact-us-reason-select')} select`,
	);
	reason.selectedIndex = 1;
	reason.dispatchEvent(new Event('input', { bubbles: true }));
	reason.dispatchEvent(new Event('change', { bubbles: true }));
}

const contactUs = '/settings/help/contact-us';

/** Open the form and pick `files` as the screenshots to send with it. */
async function pickAsScreenshots(
	files: Parameters<typeof window.__test.interceptFilePickers>[0],
): Promise<void> {
	await open(contactUs);
	window.__test.interceptFilePickers(files);
	await settled();
	await tap('contact-us-screenshots-add');
}

async function sendFeedback(): Promise<void> {
	await open(contactUs);
	await chooseAReason();
	await settled();
	await fill('contact-us-message-input', 'The app is great');
	await tap('contact-us-send-btn');
}

export default [
	{ name: 'contact-us', setup: () => open(contactUs) },
	{
		name: 'contact-us-with-screenshots',
		setup: () =>
			pickAsScreenshots([
				{ name: 'crash.png', mimeType: 'image/png', bytes: TINY_PNG_BYTES },
			]),
	},
	{
		name: 'error-feedback-unsupported-image',
		setup: () =>
			pickAsScreenshots([
				{ name: 'notes.txt', mimeType: 'text/plain', bytes: [104, 105] },
			]),
	},
	{
		name: 'error-feedback-screenshot-too-large',
		setup: () =>
			pickAsScreenshots([
				{ name: 'big.gif', mimeType: 'image/gif', size: 9 * 1024 * 1024 },
			]),
	},
	{
		name: 'error-feedback-too-many-screenshots',
		setup: () =>
			pickAsScreenshots(
				['a', 'b', 'c', 'd', 'e', 'f'].map(name => ({
					name: `${name}.png`,
					mimeType: 'image/png',
					bytes: TINY_PNG_BYTES,
				})),
			),
	},
	{ name: 'contact-us-sent', setup: sendFeedback },
	{
		name: 'contact-us-queued',
		async setup() {
			setMockBackend({
				commandAnswers: {
					'plugin:sentry-reporting|send_feedback': 'queued',
				},
			});
			await sendFeedback();
		},
	},
	{
		name: 'contact-us-unavailable',
		async setup() {
			reportingIsUnavailable();
			await open(contactUs);
		},
	},
] satisfies Scene[];
