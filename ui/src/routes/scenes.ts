import { scene } from '$lib/scenes';
import {
	ALICE,
	BOB,
	CAROL,
	DAVE,
	directChatId,
	nobodyHasSetUpThisDevice,
	onlyMyProfile,
} from '$lib/scenes/demo-world';
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

function connecting(): void {
	setMockBackend({
		mailboxes: {
			...mockBackend().mailboxes,
			cloudMailbox: MOCK_MAILBOX_UNCONFIRMED,
		},
	});
}

function isolated(): void {
	setMockBackend({
		mailboxes: { ...mockBackend().mailboxes, cloudMailbox: MOCK_MAILBOX_DOWN },
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
		mailboxes: { ...mockBackend().mailboxes, cloudMailbox: MOCK_MAILBOX_DOWN },
		onlinePeers: [ALICE.agentId],
	});
}

scene('welcome', () => {
	nobodyHasSetUpThisDevice();
});

scene('create-profile', async ({ page, open }) => {
	nobodyHasSetUpThisDevice();
	await open('/');
	await page.getByTestId('welcome-continue-btn').first().click();
});

scene('home-empty', () => {
	onlyMyProfile();
});

scene('home');

scene('home-chat-summaries', () => {
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
});

scene('ios-home', undefined, { device: 'iphone' });

scene('desktop-home', undefined, { device: 'desktop' });

scene('connection-modes', async ({ page, open }) => {
	await open('/');
	await page.getByTestId('connection-status').first().click();
});

scene('connection-modes-connecting', async ({ page, open }) => {
	connecting();
	await open('/');
	await page.getByTestId('connection-status').first().click();
});

scene('home-disconnected', () => {
	isolated();
});

scene('connection-modes-isolated', async ({ page, open }) => {
	isolated();
	await open('/');
	await page.getByTestId('connection-status').first().click();
});

scene('home-on-a-local-hub', () => {
	onALocalHub();
});

scene('connection-modes-local-hub', async ({ page, open }) => {
	onALocalHub();
	await open('/');
	await page.getByTestId('connection-status').first().click();
});

scene('connection-mode-local-hub-help', async ({ page, open }) => {
	await open('/');
	await page.getByTestId('connection-status').first().click();
	await page.getByTestId('connection-mode-local-hub').first().click();
});

scene('home-phone-to-phone', () => {
	phoneToPhone();
});

scene('connection-modes-phone-to-phone', async ({ page, open }) => {
	phoneToPhone();
	await open('/');
	await page.getByTestId('connection-status').first().click();
});

scene('connection-mode-phone-to-phone-help', async ({ page, open }) => {
	await open('/');
	await page.getByTestId('connection-status').first().click();
	await page.getByTestId('connection-mode-phone-to-phone').first().click();
});

scene(
	'desktop-updater-available',
	async ({ page, open }) => {
		await open('/');
		await page.evaluate(() => window.__test.simulateUpdate('available'));
	},
	{ device: 'desktop' },
);

scene(
	'desktop-updater-downloading',
	async ({ page, open }) => {
		await open('/');
		await page.evaluate(() => window.__test.simulateUpdate('downloading'));
	},
	{ device: 'desktop' },
);

scene(
	'desktop-updater-ready',
	async ({ page, open }) => {
		await open('/');
		await page.evaluate(() => window.__test.simulateUpdate('ready'));
	},
	{ device: 'desktop' },
);

scene(
	'desktop-updater-error',
	async ({ page, open }) => {
		await open('/');
		await page.evaluate(() => window.__test.simulateUpdate('error'));
	},
	{ device: 'desktop' },
);

scene(
	'desktop-updater-misplaced',
	async ({ page, open }) => {
		await open('/');
		await page.evaluate(() => window.__test.simulateUpdate('misplaced'));
	},
	{ device: 'desktop' },
);

scene(
	'desktop-updater-moving',
	async ({ page, open }) => {
		await open('/');
		await page.evaluate(() => window.__test.simulateUpdate('moving'));
	},
	{ device: 'desktop' },
);

scene(
	'desktop-updater-move-error',
	async ({ page, open }) => {
		await open('/');
		await page.evaluate(() => window.__test.simulateUpdate('moveError'));
	},
	{ device: 'desktop' },
);

scene('error-loading-chats', () => {
	setMockBackend({ failingCommands: ['get_group_chats'] });
});

scene('error-report-dialog', async ({ page, open }) => {
	setMockBackend({ failingCommands: ['get_group_chats'] });
	await open('/');
	await page.getByTestId('error-placeholder-send-report').first().click();
});

scene('error-sending-error-report', async ({ page, open }) => {
	setMockBackend({
		failingCommands: [
			'get_group_chats',
			'plugin:sentry-reporting|send_error_report',
		],
	});
	await open('/');
	await page.getByTestId('error-placeholder-send-report').first().click();
	await page.getByTestId('send-error-report-confirm').first().click();
});

scene('crash-report-dialog', () => {
	setMockBackend({
		commandAnswers: {
			'plugin:sentry-reporting|pending_crash_report': true,
		},
	});
});
