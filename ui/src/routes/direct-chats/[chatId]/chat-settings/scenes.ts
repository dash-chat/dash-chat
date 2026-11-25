import {
	ALICE,
	deviceGroupOperation,
	directChatPath,
} from '$lib/scenes/demo-world';
import type { Scene } from '$lib/scenes/scene';
import { minutesAgo, mockBackend } from 'dash-chat-stores';

const settings = `${directChatPath(ALICE)}/chat-settings`;

function blockAlice(): void {
	deviceGroupOperation(
		{ type: 'BlockAgent', payload: ALICE.agentId },
		minutesAgo(1),
	);
}

export default [
	{ name: 'chat-settings', path: settings },
	{
		name: 'block-contact-dialog',
		path: settings,
		steps: [{ tap: 'chat-settings-block-btn' }],
	},
	{
		name: 'report-contact-dialog',
		path: settings,
		steps: [{ tap: 'chat-settings-report-btn' }],
	},
	{ name: 'chat-settings-blocked', path: settings, seed: blockAlice },
	{
		name: 'unblock-contact-dialog',
		path: settings,
		seed: blockAlice,
		steps: [{ tap: 'chat-settings-unblock-btn' }],
	},
	{
		name: 'block-contact-result',
		path: settings,
		steps: [
			{ tap: 'chat-settings-block-btn' },
			{ tap: 'block-contact-confirm' },
		],
	},
	{
		name: 'unblock-contact-result',
		path: settings,
		seed: blockAlice,
		steps: [
			{ tap: 'chat-settings-unblock-btn' },
			{ tap: 'unblock-contact-confirm' },
		],
	},
	{
		name: 'report-contact-result',
		path: settings,
		steps: [
			{ tap: 'chat-settings-report-btn' },
			{ tap: 'report-contact-confirm' },
		],
	},
	{
		name: 'report-contact-failed',
		path: settings,
		seed() {
			mockBackend.failingCommands = ['report_contact'];
		},
		steps: [
			{ tap: 'chat-settings-report-btn' },
			{ tap: 'report-contact-confirm' },
		],
	},
] satisfies Scene[];
