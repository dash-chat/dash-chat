import { field, hold, scene, untilAToastShows } from '$lib/scenes';
import {
	ALICE,
	BOB,
	STRANGER,
	TINY_PNG_BYTES,
	directChatId,
	directChatPath,
	pasteIntoComposer,
} from '$lib/scenes/demo-world';
import type { Page } from '@playwright/test';
import {
	DEMO_CONTACT_REQUESTER,
	DEMO_IDS,
	chatOperation,
	deviceGroupOperation,
	minutesAgo,
	setMockBackend,
	textMessage,
} from 'dash-chat-mock';

import { tid } from '../../../../tests/selectors';

const chat = directChatPath(ALICE);
const request = directChatPath(DEMO_CONTACT_REQUESTER);

/** Press the record button and keep it down, as a thumb does. */
async function pressAndHoldRecord(page: Page): Promise<void> {
	// A synthetic pointer is not one the browser can capture.
	await page.evaluate(() => {
		Element.prototype.setPointerCapture = () => {};
	});
	await page
		.locator(`${tid('message-input-voice-record')}:visible`)
		.first()
		.dispatchEvent('pointerdown', {
			pointerId: 1,
			pointerType: 'touch',
			clientX: 300,
			clientY: 800,
		});
}

async function openFirstPhoto(page: Page): Promise<void> {
	await page
		.getByTestId('message-attachment-photos')
		.first()
		.getByRole('button')
		.first()
		.click();
}

function mediaMessages(): void {
	const chatId = directChatId(ALICE);
	chatOperation(
		ALICE.deviceId,
		chatId,
		{
			type: 'Message',
			payload: {
				message: '',
				media: [
					{
						kind: 'File',
						name: 'trail-map.pdf',
						mime_type: 'application/pdf',
						size: 482_000,
						hash: 'f1'.repeat(32),
					},
				],
			},
		},
		minutesAgo(4),
	);
	chatOperation(
		DEMO_IDS.MY_DEVICE_ID,
		chatId,
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
						waveform: new Uint8Array(48).fill(90),
						hash: 'f2'.repeat(32),
					},
				],
			},
		},
		minutesAgo(3),
	);
	chatOperation(
		ALICE.deviceId,
		chatId,
		{
			type: 'Message',
			payload: {
				message: 'From the summit',
				media: [
					{
						kind: 'Photo',
						name: 'summit.jpg',
						mime_type: 'image/jpeg',
						size: 310_000,
						width: 1200,
						height: 900,
						hash: 'f3'.repeat(32),
					},
					{
						kind: 'Photo',
						name: 'lake.jpg',
						mime_type: 'image/jpeg',
						size: 280_000,
						width: 1200,
						height: 900,
						hash: 'f4'.repeat(32),
					},
				],
			},
		},
		minutesAgo(2),
	);
}

scene('direct-chat', async ({ open }) => {
	await open(chat);
});

scene('direct-chat-edited-deleted-reacted', async ({ open }) => {
	const chatId = directChatId(ALICE);
	const mine = textMessage(
		DEMO_IDS.MY_DEVICE_ID,
		chatId,
		'See you at 6',
		minutesAgo(4),
	);
	chatOperation(
		DEMO_IDS.MY_DEVICE_ID,
		chatId,
		{
			type: 'EditMessage',
			payload: { message: 'See you at 7', edit_hash: mine },
		},
		minutesAgo(3),
	);
	chatOperation(
		ALICE.deviceId,
		chatId,
		{ type: 'Reaction', payload: { emoji: '👍', target: mine } },
		minutesAgo(2),
	);
	const theirs = textMessage(
		ALICE.deviceId,
		chatId,
		'Wrong chat',
		minutesAgo(2),
	);
	chatOperation(
		ALICE.deviceId,
		chatId,
		{ type: 'DeleteMessage', payload: { hashes: [theirs] } },
		minutesAgo(1),
	);
	const retracted = textMessage(
		DEMO_IDS.MY_DEVICE_ID,
		chatId,
		'Never mind',
		minutesAgo(1),
	);
	chatOperation(
		DEMO_IDS.MY_DEVICE_ID,
		chatId,
		{ type: 'DeleteMessage', payload: { hashes: [retracted] } },
		minutesAgo(0),
	);
	await open(chat);
});

scene('direct-chat-blocked', async ({ open }) => {
	deviceGroupOperation(
		{ type: 'BlockAgent', payload: ALICE.agentId },
		minutesAgo(1),
	);
	await open(chat);
});

scene('direct-chat-reply-and-long-message', async ({ open }) => {
	const chatId = directChatId(ALICE);
	const question = textMessage(
		ALICE.deviceId,
		chatId,
		'What should I bring?',
		minutesAgo(4),
	);
	chatOperation(
		DEMO_IDS.MY_DEVICE_ID,
		chatId,
		{
			type: 'Message',
			payload: {
				message: `Here is the full list. ${'Water, snacks, a map, sunscreen, a hat. '.repeat(30)}`,
				media: null,
				reply: question,
			},
		},
		minutesAgo(3),
	);
	await open(chat);
});

scene('message-sending', async ({ open }) => {
	setMockBackend({ operationsReachMailboxes: false });
	textMessage(
		DEMO_IDS.MY_DEVICE_ID,
		directChatId(ALICE),
		'Are you there?',
		minutesAgo(5),
	);
	await open(chat);
});

scene('message-unsent', async ({ open }) => {
	setMockBackend({ operationsReachMailboxes: false });
	textMessage(
		DEMO_IDS.MY_DEVICE_ID,
		directChatId(ALICE),
		'Are you there?',
		minutesAgo(0),
	);
	await open(chat);
});

scene('contact-request', async ({ open }) => {
	await open(request);
});

scene('accept-contact-request-dialog', async ({ page, open }) => {
	await open(request);
	await page.getByTestId('direct-chat-accept-btn').first().click();
});

for (const tip of [1, 2, 3, 4, 5]) {
	scene(`security-tips-${tip}`, async ({ page, open }) => {
		await open(request);
		await page.getByTestId('direct-chat-security-tips').first().click();
		for (let turned = 1; turned < tip; turned++) {
			await page.getByTestId('security-tips-next').first().click();
		}
	});
}

scene('message-actions', async ({ page, open }) => {
	await open(chat);
	await hold(page, 'message-from-me');
});

scene('delete-message-dialog', async ({ page, open }) => {
	await open(chat);
	await hold(page, 'message-from-me');
	await page.getByTestId('message-action-delete').first().click();
});

scene(
	'ios-direct-chat',
	async ({ open }) => {
		await open(chat);
	},
	{ device: 'iphone' },
);

scene('message-editing', async ({ page, open }) => {
	await open(chat);
	await hold(page, 'message-from-me');
	await page.getByTestId('message-action-edit').first().click();
});

scene('discard-draft-dialog', async ({ page, open }) => {
	await open(chat);
	await field(page, 'message-input-textarea').fill('Draft in progress');
	await hold(page, 'message-from-me');
	await page.getByTestId('message-action-edit').first().click();
});

scene('message-replying', async ({ page, open }) => {
	await open(chat);
	await hold(page, 'message-from-me');
	await page.getByTestId('message-action-reply').first().click();
});

scene('message-reactions', async ({ page, open }) => {
	const chatId = directChatId(ALICE);
	const mine = textMessage(
		DEMO_IDS.MY_DEVICE_ID,
		chatId,
		'Shall we go?',
		minutesAgo(4),
	);
	chatOperation(
		ALICE.deviceId,
		chatId,
		{ type: 'Reaction', payload: { emoji: '👍', target: mine } },
		minutesAgo(3),
	);
	chatOperation(
		DEMO_IDS.MY_DEVICE_ID,
		chatId,
		{ type: 'Reaction', payload: { emoji: '👍', target: mine } },
		minutesAgo(2),
	);
	await open(chat);
	await page.getByTestId('reaction-chip-👍').first().click();
});

scene('direct-chat-media', async ({ open }) => {
	mediaMessages();
	await open(chat);
});

scene('attach-panel', async ({ page, open }) => {
	await open(chat);
	await page
		.getByTestId('message-input-attach')
		.filter({ visible: true })
		.click();
});

scene('search-results', async ({ page, open }) => {
	await open(`${chat}?search=true`);
	await field(page, 'direct-chat-search-input').fill('offline');
});

scene('search-no-results', async ({ page, open }) => {
	await open(`${chat}?search=true`);
	await field(page, 'direct-chat-search-input').fill('zzzz');
});

scene('direct-chat-reported', async ({ open }) => {
	deviceGroupOperation(
		{ type: 'BlockAgent', payload: ALICE.agentId },
		minutesAgo(3),
	);
	deviceGroupOperation(
		{ type: 'UnblockAgent', payload: ALICE.agentId },
		minutesAgo(2),
	);
	deviceGroupOperation(
		{
			type: 'ReportContact',
			payload: { agent_id: ALICE.agentId, device_ids: [], mailbox_ids: [] },
		},
		minutesAgo(1),
	);
	await open(chat);
});

scene(
	'desktop-direct-chat',
	async ({ open }) => {
		await open(chat);
	},
	{ device: 'desktop' },
);

scene(
	'desktop-message-toolbar',
	async ({ page, open }) => {
		await open(chat);
		await page.getByTestId('message-from-me').first().hover();
	},
	{ device: 'desktop' },
);

scene('photo-lightbox', async ({ page, open }) => {
	mediaMessages();
	await open(chat);
	await openFirstPhoto(page);
});

scene('error-voice-play', async ({ page, open }) => {
	mediaMessages();
	await open(chat);
	await page.getByTestId('voice-play-button').first().click();
});

scene('staged-photo', async ({ page, open }) => {
	await open(chat);
	await pasteIntoComposer(page, [
		{ name: 'summit.png', mimeType: 'image/png', bytes: TINY_PNG_BYTES },
	]);
});

scene('staged-file', async ({ page, open }) => {
	await open(chat);
	await pasteIntoComposer(page, [
		{ name: 'trail-map.pdf', mimeType: 'application/pdf', size: 2048 },
	]);
});

scene('error-attachment-too-large', async ({ page, open }) => {
	await open(chat);
	await pasteIntoComposer(page, [
		{
			name: 'film.bin',
			mimeType: 'application/octet-stream',
			size: 17 * 1024 * 1024,
		},
	]);
	// The composer's own send button is under the page the file is staged on.
	await page
		.getByTestId('staged-media-page')
		.getByTestId('message-input-send')
		.click();
	await untilAToastShows(page);
});

scene('error-one-file-at-a-time', async ({ page, open }) => {
	await open(chat);
	await pasteIntoComposer(page, [
		{ name: 'a.pdf', mimeType: 'application/pdf', size: 2048 },
		{ name: 'b.pdf', mimeType: 'application/pdf', size: 2048 },
	]);
});

scene('error-files-with-photos', async ({ page, open }) => {
	await open(chat);
	await pasteIntoComposer(page, [
		{ name: 'a.pdf', mimeType: 'application/pdf', size: 2048 },
		{ name: 'summit.png', mimeType: 'image/png', bytes: TINY_PNG_BYTES },
	]);
});

scene('error-too-many-attachments', async ({ page, open }) => {
	await open(chat);
	await pasteIntoComposer(
		page,
		Array.from({ length: 40 }, (_, index) => ({
			name: `photo-${index}.png`,
			mimeType: 'image/png',
			bytes: TINY_PNG_BYTES,
		})),
	);
	await untilAToastShows(page);
});

scene('error-damaged-photo', async ({ page, open }) => {
	await open(chat);
	await pasteIntoComposer(page, [
		{
			name: 'damaged.jpg',
			mimeType: 'image/jpeg',
			bytes: [255, 216, 255],
		},
	]);
	await untilAToastShows(page);
});

scene('error-unreadable-photo', async ({ page, open }) => {
	await open(chat);
	await page.getByTestId('message-input-textarea').first().waitFor();
	await page.evaluate(() => window.__test.pasteUnreadablePhoto('withdrawn'));
	await untilAToastShows(page);
});

scene('error-unreadable-file', async ({ page, open }) => {
	await open(chat);
	await page.getByTestId('message-input-textarea').first().waitFor();
	await page.evaluate(() => window.__test.pasteUnreadableFile('withdrawn.txt'));
	await untilAToastShows(page);
});

scene('error-sending-message', async ({ page, open }) => {
	setMockBackend({ failingCommands: ['send_message'] });
	await open(chat);
	await field(page, 'message-input-textarea').fill('On my way');
	await page.getByTestId('message-input-send').first().click();
});

scene('recent-photos-permission', async ({ page, open }) => {
	await open(chat);
	await page.evaluate(() => {
		window.__test.recentPhotos = { permission: 'prompt', photos: [] };
	});
	await page
		.getByTestId('message-input-attach')
		.filter({ visible: true })
		.click();
});

scene(
	'ios-recent-photos-no-access',
	async ({ page, open }) => {
		await open(chat);
		await page.evaluate(() => {
			window.__test.recentPhotos = { permission: 'denied', photos: [] };
		});
		await page
			.getByTestId('message-input-attach')
			.filter({ visible: true })
			.click();
	},
	{ device: 'iphone' },
);

scene('direct-chat-yesterday', async ({ open }) => {
	textMessage(ALICE.deviceId, directChatId(BOB), 'unused', minutesAgo(0));
	textMessage(
		ALICE.deviceId,
		directChatId(ALICE),
		'Good night!',
		minutesAgo(26 * 60),
	);
	await open(chat);
});

scene('contact-request-with-messages', async ({ open }) => {
	for (const [message, age] of [
		['Hi, we met at the meetup', 9],
		['Is this the right number?', 8],
	] as const) {
		textMessage(
			DEMO_CONTACT_REQUESTER.deviceId,
			directChatId(DEMO_CONTACT_REQUESTER),
			message,
			minutesAgo(age),
		);
	}
	await open(request);
});

scene('direct-chat-scrolled-up', async ({ page, open }) => {
	for (let index = 0; index < 40; index++) {
		textMessage(
			ALICE.deviceId,
			directChatId(ALICE),
			`Trail note ${index + 1}`,
			minutesAgo(4),
		);
	}
	await open(chat);
	await page
		.getByTestId('direct-chat-scroll')
		.evaluate(scroll => scroll.scrollTo({ top: -100_000 }));
});

scene(
	'desktop-photo-lightbox',
	async ({ page, open }) => {
		mediaMessages();
		await open(chat);
		await openFirstPhoto(page);
	},
	{ device: 'desktop' },
);

scene(
	'desktop-staged-file',
	async ({ page, open }) => {
		await open(chat);
		await pasteIntoComposer(page, [
			{ name: 'trail-map.pdf', mimeType: 'application/pdf', size: 2048 },
		]);
	},
	{ device: 'desktop' },
);

scene(
	'desktop-staged-photos',
	async ({ page, open }) => {
		await open(chat);
		await pasteIntoComposer(page, [
			{ name: 'summit.png', mimeType: 'image/png', bytes: TINY_PNG_BYTES },
			{ name: 'lake.png', mimeType: 'image/png', bytes: TINY_PNG_BYTES },
		]);
		await page.getByTestId('message-input-clear-attachments').waitFor();
	},
	{ device: 'desktop' },
);

scene('direct-chat-waiting-for-profile', async ({ open }) => {
	deviceGroupOperation(
		{
			type: 'AddContact',
			payload: {
				agent_id: STRANGER.agentId,
				direct_chat_topic_id: directChatId(STRANGER),
			},
		},
		minutesAgo(1),
	);
	await open(directChatPath(STRANGER));
});

scene('profile-names', async ({ page, open }) => {
	await open(chat);
	await page.getByTestId('direct-chat-name-not-verified').first().click();
});

scene('message-delivered', async ({ open }) => {
	setMockBackend({
		messageAcks: {
			[DEMO_IDS.MY_DEVICE_ID]: { hash: '', seq: Number.MAX_SAFE_INTEGER },
		},
	});
	await open(chat);
});

scene(
	'desktop-attach-menu',
	async ({ page, open }) => {
		await open(chat);
		await page
			.getByTestId('message-input-attach')
			.filter({ visible: true })
			.click();
	},
	{ device: 'desktop' },
);

scene(
	'desktop-drop-files',
	async ({ page, open }) => {
		await open(chat);
		await page.evaluate(() =>
			window.__test.dragFilesOver([
				{ name: 'trail-map.pdf', mimeType: 'application/pdf', size: 2048 },
			]),
		);
	},
	{ device: 'desktop' },
);

scene(
	'desktop-photo-lightbox-second',
	async ({ page, open }) => {
		mediaMessages();
		await open(chat);
		await openFirstPhoto(page);
		await page.getByTestId('lightbox-next').first().click();
	},
	{ device: 'desktop' },
);

scene('direct-chat-media-missing', async ({ open }) => {
	mediaMessages();
	setMockBackend({ mediaIsMissing: true });
	await open(chat);
});

scene('voice-message-playing', async ({ page, open }) => {
	mediaMessages();
	await open(chat);
	await page.evaluate(() => {
		HTMLMediaElement.prototype.play = function (this: HTMLMediaElement) {
			this.dispatchEvent(new Event('play'));
			return Promise.resolve();
		};
	});
	await page.getByTestId('voice-play-button').first().click();
});

scene('recent-photos-denied', async ({ page, open }) => {
	await open(chat);
	await page.evaluate(() => {
		window.__test.recentPhotos = { permission: 'prompt', photos: [] };
	});
	await page
		.getByTestId('message-input-attach')
		.filter({ visible: true })
		.click();
	await page.getByTestId('message-input-recent-photos-allow').first().click();
});

scene('recent-photos-settings', async ({ page, open }) => {
	await open(chat);
	await page.evaluate(() => {
		window.__test.recentPhotos = { permission: 'prompt', photos: [] };
	});
	await page
		.getByTestId('message-input-attach')
		.filter({ visible: true })
		.click();
	await page.getByTestId('message-input-recent-photos-allow').first().click();
	await page.getByTestId('message-input-recent-photos-allow').first().click();
});

scene('error-downloading-file', async ({ page, open }) => {
	mediaMessages();
	setMockBackend({ mediaIsMissing: true });
	await open(chat);
	await page.getByTestId('message-attachment-file').first().click();
});

scene('error-loading-recent-photos', async ({ page, open }) => {
	await open(chat);
	await page.evaluate(() => {
		window.__test.recentPhotos = {
			permission: 'granted',
			get photos(): never {
				throw new Error('the photo library cannot be read');
			},
		};
	});
	await page
		.getByTestId('message-input-attach')
		.filter({ visible: true })
		.click();
});

scene('error-adding-recent-photo', async ({ page, open }) => {
	await open(chat);
	await page.evaluate(() => {
		window.__test.recentPhotos = {
			permission: 'granted',
			photos: [
				{
					id: 'gone',
					name: 'gone.png',
					mimeType: 'image/png',
					dataUrl: 'http://localhost:1/gone.png',
				},
			],
		};
	});
	await page
		.getByTestId('message-input-attach')
		.filter({ visible: true })
		.click();
	await page.getByTestId('message-input-recent-photo-0').first().click();
	await untilAToastShows(page);
});

scene('voice-recording', async ({ page, open }) => {
	setMockBackend({
		commandAnswers: {
			'plugin:audio-recorder|check_permission': { granted: true },
		},
	});
	await open(chat);
	await pressAndHoldRecord(page);
});

scene('error-microphone-not-allowed', async ({ page, open }) => {
	setMockBackend({
		commandAnswers: {
			'plugin:audio-recorder|check_permission': { granted: false },
		},
	});
	await open(chat);
	await pressAndHoldRecord(page);
	await untilAToastShows(page);
});

scene('error-recording-voice', async ({ page, open }) => {
	setMockBackend({
		commandAnswers: {
			'plugin:audio-recorder|check_permission': { granted: true },
		},
		failingCommands: ['plugin:audio-recorder|start_recording'],
	});
	await open(chat);
	await pressAndHoldRecord(page);
	await untilAToastShows(page);
});

scene(
	'desktop-file-saved',
	async ({ page, open }) => {
		mediaMessages();
		setMockBackend({
			commandAnswers: {
				'plugin:dialog|save': '/home/me/trail-map.pdf',
			},
		});
		await open(chat);
		await page.evaluate(() => {
			window.__TAURI_INTERNALS__!.preview = false;
		});
		await page.getByTestId('message-attachment-file').first().click();
		await untilAToastShows(page);
	},
	{ device: 'desktop' },
);

scene(
	'desktop-photo-saved',
	async ({ page, open }) => {
		mediaMessages();
		setMockBackend({
			commandAnswers: {
				'plugin:dialog|save': '/home/me/summit.jpg',
			},
		});
		await open(chat);
		await openFirstPhoto(page);
		await page.evaluate(() => {
			window.__TAURI_INTERNALS__!.preview = false;
		});
		await page.getByTestId('lightbox-save').first().click();
		await untilAToastShows(page);
	},
	{ device: 'desktop' },
);
