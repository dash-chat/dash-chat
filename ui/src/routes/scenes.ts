import {
	ALICE,
	BOB,
	CAROL,
	DAVE,
	chatOperation,
	deviceGroupOperation,
	directChatId,
	onlyMyProfile,
} from '$lib/scenes/demo-world';
import type { Scene } from '$lib/scenes/scene';
import {
	MOCK_MAILBOX_DOWN,
	MOCK_MAILBOX_UNCONFIRMED,
	MOCK_MAILBOX_UP,
	minutesAgo,
	mockBackend,
} from 'dash-chat-stores';

function connecting(): void {
	mockBackend.mailboxes.cloudMailbox = MOCK_MAILBOX_UNCONFIRMED;
}

function isolated(): void {
	mockBackend.mailboxes.cloudMailbox = MOCK_MAILBOX_DOWN;
}

function onALocalHub(): void {
	mockBackend.mailboxes = {
		cloudMailbox: MOCK_MAILBOX_DOWN,
		localMailboxes: [MOCK_MAILBOX_UP],
	};
}

function phoneToPhone(): void {
	mockBackend.mailboxes.cloudMailbox = MOCK_MAILBOX_DOWN;
	mockBackend.onlinePeers = [ALICE.agentId];
}

export default [
	{ name: 'welcome', path: '/', firstLaunch: true },
	{
		name: 'create-profile',
		path: '/',
		firstLaunch: true,
		steps: [{ tap: 'welcome-continue-btn' }],
	},
	{
		name: 'home-empty',
		path: '/',
		firstLaunch: true,
		seed: onlyMyProfile,
	},
	{ name: 'home', path: '/' },
	{
		name: 'home-chat-summaries',
		path: '/',
		seed() {
			chatOperation(
				ALICE.deviceId,
				directChatId(ALICE),
				{
					type: 'Message',
					payload: {
						message: '',
						media: ['summit', 'lake'].map((name, index) => ({
							kind: 'Photo' as const,
							name: `${name}.jpg`,
							mime_type: 'image/jpeg',
							size: 300_000,
							width: 1200,
							height: 900,
							hash: `a${index}`.repeat(32),
						})),
					},
				},
				minutesAgo(4),
			);
			chatOperation(
				BOB.deviceId,
				directChatId(BOB),
				{
					type: 'Message',
					payload: {
						message: '',
						media: [
							{
								kind: 'VoiceNote',
								mime_type: 'audio/ogg',
								size: 12_000,
								duration_ms: 7_000,
								waveform: new Uint8Array(48),
								hash: 'b0'.repeat(32),
							},
						],
					},
				},
				minutesAgo(3),
			);
			const retracted = chatOperation(
				CAROL.deviceId,
				directChatId(CAROL),
				{ type: 'Message', payload: { message: 'Oops', media: null } },
				minutesAgo(2),
			);
			chatOperation(
				CAROL.deviceId,
				directChatId(CAROL),
				{ type: 'DeleteMessage', payload: { hashes: [retracted] } },
				minutesAgo(1),
			);
			deviceGroupOperation(
				{ type: 'BlockAgent', payload: DAVE.agentId },
				minutesAgo(1),
			);
		},
	},
	{ name: 'ios-home', path: '/', device: 'iphone' },
	{ name: 'desktop-home', path: '/', device: 'desktop' },
	{
		name: 'connection-modes',
		path: '/',
		steps: [{ tap: 'connection-status' }],
	},
	{
		name: 'connection-modes-connecting',
		path: '/',
		seed: connecting,
		steps: [{ tap: 'connection-status' }],
	},
	{ name: 'home-disconnected', path: '/', seed: isolated },
	{
		name: 'connection-modes-isolated',
		path: '/',
		seed: isolated,
		steps: [{ tap: 'connection-status' }],
	},
	{ name: 'home-on-a-local-hub', path: '/', seed: onALocalHub },
	{
		name: 'connection-modes-local-hub',
		path: '/',
		seed: onALocalHub,
		steps: [{ tap: 'connection-status' }],
	},
	{
		name: 'connection-mode-local-hub-help',
		path: '/',
		steps: [{ tap: 'connection-status' }, { tap: 'connection-mode-local-hub' }],
	},
	{ name: 'home-phone-to-phone', path: '/', seed: phoneToPhone },
	{
		name: 'connection-modes-phone-to-phone',
		path: '/',
		seed: phoneToPhone,
		steps: [{ tap: 'connection-status' }],
	},
	{
		name: 'connection-mode-phone-to-phone-help',
		path: '/',
		steps: [
			{ tap: 'connection-status' },
			{ tap: 'connection-mode-phone-to-phone' },
		],
	},
	{
		name: 'desktop-updater-available',
		path: '/',
		device: 'desktop',
		steps: [() => window.__test.simulateUpdate('available')],
	},
	{
		name: 'desktop-updater-downloading',
		path: '/',
		device: 'desktop',
		steps: [() => window.__test.simulateUpdate('downloading')],
	},
	{
		name: 'desktop-updater-ready',
		path: '/',
		device: 'desktop',
		steps: [() => window.__test.simulateUpdate('ready')],
	},
	{
		name: 'desktop-updater-error',
		path: '/',
		device: 'desktop',
		steps: [() => window.__test.simulateUpdate('error')],
	},
	{
		name: 'desktop-updater-misplaced',
		path: '/',
		device: 'desktop',
		steps: [() => window.__test.simulateUpdate('misplaced')],
	},
	{
		name: 'desktop-updater-moving',
		path: '/',
		device: 'desktop',
		steps: [() => window.__test.simulateUpdate('moving')],
	},
	{
		name: 'desktop-updater-move-error',
		path: '/',
		device: 'desktop',
		steps: [() => window.__test.simulateUpdate('moveError')],
	},
	{
		name: 'error-loading-chats',
		path: '/',
		seed() {
			mockBackend.failingCommands = ['get_group_chats'];
		},
	},
	{
		name: 'error-report-dialog',
		path: '/',
		seed() {
			mockBackend.failingCommands = ['get_group_chats'];
		},
		steps: [{ tap: 'error-placeholder-send-report' }],
	},
	{
		name: 'error-sending-error-report',
		path: '/',
		seed() {
			mockBackend.failingCommands = [
				'get_group_chats',
				'plugin:sentry-reporting|send_error_report',
			];
		},
		steps: [
			{ tap: 'error-placeholder-send-report' },
			{ tap: 'send-error-report-confirm' },
		],
	},
	{
		name: 'crash-report-dialog',
		path: '/',
		seed() {
			mockBackend.commandAnswers = {
				'plugin:sentry-reporting|pending_crash_report': true,
			};
		},
	},
] satisfies Scene[];
