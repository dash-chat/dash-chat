import {
	ALICE,
	deviceGroupOperation,
	directChatPath,
} from '$lib/scenes/demo-world';
import type { Scene } from '$lib/scenes/scene';
import { open } from '$lib/scenes/steps';
import { minutesAgo, setMockBackend } from 'dash-chat-mock';
import { tap } from 'translation-screenshots/scene';

const settings = `${directChatPath(ALICE)}/chat-settings`;

function blockAlice(): void {
	deviceGroupOperation(
		{ type: 'BlockAgent', payload: ALICE.agentId },
		minutesAgo(1),
	);
}

export default [
	{
		name: 'chat-settings',
		setup: () => open(settings),
	},
	{
		name: 'block-contact-dialog',
		async setup() {
			await open(settings);
			await tap('chat-settings-block-btn');
		},
	},
	{
		name: 'report-contact-dialog',
		async setup() {
			await open(settings);
			await tap('chat-settings-report-btn');
		},
	},
	{
		name: 'chat-settings-blocked',
		async setup() {
			blockAlice();
			await open(settings);
		},
	},
	{
		name: 'unblock-contact-dialog',
		async setup() {
			blockAlice();
			await open(settings);
			await tap('chat-settings-unblock-btn');
		},
	},
	{
		name: 'block-contact-result',
		async setup() {
			await open(settings);
			await tap('chat-settings-block-btn');
			await tap('block-contact-confirm');
		},
	},
	{
		name: 'unblock-contact-result',
		async setup() {
			blockAlice();
			await open(settings);
			await tap('chat-settings-unblock-btn');
			await tap('unblock-contact-confirm');
		},
	},
	{
		name: 'report-contact-result',
		async setup() {
			await open(settings);
			await tap('chat-settings-report-btn');
			await tap('report-contact-confirm');
		},
	},
	{
		name: 'report-contact-failed',
		async setup() {
			setMockBackend({ failingCommands: ['report_contact'] });
			await open(settings);
			await tap('chat-settings-report-btn');
			await tap('report-contact-confirm');
		},
	},
] satisfies Scene[];
