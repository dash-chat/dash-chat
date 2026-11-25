import { reportingIsUnavailable } from '$lib/scenes/demo-world';
import type { Scene } from '$lib/scenes/scene';
import { mockBackend } from 'dash-chat-stores';

const TINY_PNG = [
	137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0,
	0, 0, 1, 8, 6, 0, 0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 120,
	156, 99, 248, 207, 192, 240, 31, 0, 5, 0, 1, 255, 137, 153, 61, 29, 0, 0, 0,
	0, 73, 69, 78, 68, 174, 66, 96, 130,
];

export default [
	{ name: 'contact-us', path: '/settings/help/contact-us' },
	{
		name: 'contact-us-with-screenshots',
		path: '/settings/help/contact-us',
		steps: [
			() =>
				window.__test.interceptFilePickers([
					{ name: 'crash.png', mimeType: 'image/png', bytes: TINY_PNG },
				]),
			{ tap: 'contact-us-screenshots-add' },
		],
	},
	{
		name: 'error-feedback-unsupported-image',
		path: '/settings/help/contact-us',
		steps: [
			() =>
				window.__test.interceptFilePickers([
					{ name: 'notes.txt', mimeType: 'text/plain', bytes: [104, 105] },
				]),
			{ tap: 'contact-us-screenshots-add' },
		],
	},
	{
		name: 'error-feedback-screenshot-too-large',
		path: '/settings/help/contact-us',
		steps: [
			() =>
				window.__test.interceptFilePickers([
					{ name: 'big.gif', mimeType: 'image/gif', size: 9 * 1024 * 1024 },
				]),
			{ tap: 'contact-us-screenshots-add' },
		],
	},
	{
		name: 'error-feedback-too-many-screenshots',
		path: '/settings/help/contact-us',
		steps: [
			() =>
				window.__test.interceptFilePickers(
					['a', 'b', 'c', 'd', 'e', 'f'].map(name => ({
						name: `${name}.png`,
						mimeType: 'image/png',
						bytes: TINY_PNG,
					})),
				),
			{ tap: 'contact-us-screenshots-add' },
		],
	},
	{
		name: 'contact-us-sent',
		path: '/settings/help/contact-us',
		steps: [
			() => {
				const reason = document.querySelector<HTMLSelectElement>(
					'[data-testid="contact-us-reason-select"] select',
				);
				if (reason === null) return;
				reason.selectedIndex = 1;
				reason.dispatchEvent(new Event('input', { bubbles: true }));
				reason.dispatchEvent(new Event('change', { bubbles: true }));
			},
			{ type: 'The app is great', into: 'contact-us-message-input' },
			{ tap: 'contact-us-send-btn' },
		],
	},
	{
		name: 'contact-us-queued',
		path: '/settings/help/contact-us',
		seed() {
			mockBackend.commandAnswers = {
				'plugin:sentry-reporting|send_feedback': 'queued',
			};
		},
		steps: [
			() => {
				const reason = document.querySelector<HTMLSelectElement>(
					'[data-testid="contact-us-reason-select"] select',
				);
				if (reason === null) return;
				reason.selectedIndex = 1;
				reason.dispatchEvent(new Event('input', { bubbles: true }));
				reason.dispatchEvent(new Event('change', { bubbles: true }));
			},
			{ type: 'The app is great', into: 'contact-us-message-input' },
			{ tap: 'contact-us-send-btn' },
		],
	},
	{
		name: 'contact-us-unavailable',
		path: '/settings/help/contact-us',
		seed: reportingIsUnavailable,
	},
] satisfies Scene[];
