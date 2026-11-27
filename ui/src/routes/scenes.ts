import type { Scene } from '$lib/scenes';
import {
	ALICE,
	BOB,
	CAROL,
	DAVE,
	directChatId,
	nobodyHasSetUpThisDevice,
	onlyMyProfile,
} from '$lib/scenes/demo-world';
import { open } from '$lib/scenes/steps';
import {
	MOCK_MAILBOX_DOWN,
	MOCK_MAILBOX_UNCONFIRMED,
	MOCK_MAILBOX_UP,
	chatOperation,
	deviceGroupOperation,
	minutesAgo,
	mockBackend,
	setMockBackend,
	textMessage,
} from 'dash-chat-mock';
import { tap } from 'i18n-screenshots/scene';

function connecting(): void {
	setMockBackend({
		mailboxes: {
			...mockBackend.mailboxes,
			cloudMailbox: MOCK_MAILBOX_UNCONFIRMED,
		},
	});
}

function isolated(): void {
	setMockBackend({
		mailboxes: { ...mockBackend.mailboxes, cloudMailbox: MOCK_MAILBOX_DOWN },
	});
}

function onALocalHub(): void {
	setMockBackend({
		mailboxes: {
			cloudMailbox: MOCK_MAILBOX_DOWN,
			localMailboxes: [MOCK_MAILBOX_UP],
		},
	});
}

function phoneToPhone(): void {
	setMockBackend({
		mailboxes: { ...mockBackend.mailboxes, cloudMailbox: MOCK_MAILBOX_DOWN },
		onlinePeers: [ALICE.agentId],
	});
}

export default [
	{
		name: 'welcome',
		setup: nobodyHasSetUpThisDevice,
	},
	{
		name: 'create-profile',
		async setup() {
			nobodyHasSetUpThisDevice();
			await tap('welcome-continue-btn');
		},
	},
	{
		name: 'home-empty',
		setup: onlyMyProfile,
	},
	{
		name: 'home',
	},
	{
		name: 'home-chat-summaries',
		setup() {
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
			const retracted = textMessage(
				CAROL.deviceId,
				directChatId(CAROL),
				'Oops',
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
	{
		name: 'ios-home',
		device: 'iphone',
	},
	{
		name: 'desktop-home',
		device: 'desktop',
	},
	{
		name: 'connection-modes',
		async setup() {
			await tap('connection-status');
		},
	},
	{
		name: 'connection-modes-connecting',
		async setup() {
			connecting();
			await tap('connection-status');
		},
	},
	{
		name: 'home-disconnected',
		setup: isolated,
	},
	{
		name: 'connection-modes-isolated',
		async setup() {
			isolated();
			await tap('connection-status');
		},
	},
	{
		name: 'home-on-a-local-hub',
		setup: onALocalHub,
	},
	{
		name: 'connection-modes-local-hub',
		async setup() {
			onALocalHub();
			await tap('connection-status');
		},
	},
	{
		name: 'connection-mode-local-hub-help',
		async setup() {
			await tap('connection-status');
			await tap('connection-mode-local-hub');
		},
	},
	{
		name: 'home-phone-to-phone',
		setup: phoneToPhone,
	},
	{
		name: 'connection-modes-phone-to-phone',
		async setup() {
			phoneToPhone();
			await tap('connection-status');
		},
	},
	{
		name: 'connection-mode-phone-to-phone-help',
		async setup() {
			await tap('connection-status');
			await tap('connection-mode-phone-to-phone');
		},
	},
	{
		name: 'desktop-updater-available',
		device: 'desktop',
		async setup() {
			await open('/');
			window.__test.simulateUpdate('available');
		},
	},
	{
		name: 'desktop-updater-downloading',
		device: 'desktop',
		async setup() {
			await open('/');
			window.__test.simulateUpdate('downloading');
		},
	},
	{
		name: 'desktop-updater-ready',
		device: 'desktop',
		async setup() {
			await open('/');
			window.__test.simulateUpdate('ready');
		},
	},
	{
		name: 'desktop-updater-error',
		device: 'desktop',
		async setup() {
			await open('/');
			window.__test.simulateUpdate('error');
		},
	},
	{
		name: 'desktop-updater-misplaced',
		device: 'desktop',
		async setup() {
			await open('/');
			window.__test.simulateUpdate('misplaced');
		},
	},
	{
		name: 'desktop-updater-moving',
		device: 'desktop',
		async setup() {
			await open('/');
			window.__test.simulateUpdate('moving');
		},
	},
	{
		name: 'desktop-updater-move-error',
		device: 'desktop',
		async setup() {
			await open('/');
			window.__test.simulateUpdate('moveError');
		},
	},
	{
		name: 'error-loading-chats',
		setup() {
			setMockBackend({ failingCommands: ['get_group_chats'] });
		},
	},
	{
		name: 'error-report-dialog',
		async setup() {
			setMockBackend({ failingCommands: ['get_group_chats'] });
			await tap('error-placeholder-send-report');
		},
	},
	{
		name: 'error-sending-error-report',
		async setup() {
			setMockBackend({
				failingCommands: [
					'get_group_chats',
					'plugin:sentry-reporting|send_error_report',
				],
			});
			await tap('error-placeholder-send-report');
			await tap('send-error-report-confirm');
		},
	},
	{
		name: 'crash-report-dialog',
		setup() {
			setMockBackend({
				commandAnswers: {
					'plugin:sentry-reporting|pending_crash_report': true,
				},
			});
		},
	},
] satisfies Scene[];
