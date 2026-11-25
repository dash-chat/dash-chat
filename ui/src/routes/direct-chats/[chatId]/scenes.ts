import {
	ALICE,
	BOB,
	STRANGER,
	chatOperation,
	deviceGroupOperation,
	directChatId,
	directChatPath,
	pasteIntoComposer,
} from '$lib/scenes/demo-world';
import type { Scene } from '$lib/scenes/scene';
import { untilAToastShows } from '$lib/scenes/steps';
import { takeTheAppsOwnPaths } from '$lib/utils/preview-tauri';
import {
	DEMO_CONTACT_REQUESTER,
	DEMO_IDS,
	minutesAgo,
	mockBackend,
} from 'dash-chat-stores';

const TINY_PNG = [
	137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0,
	0, 0, 1, 8, 6, 0, 0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 120,
	156, 99, 248, 207, 192, 240, 31, 0, 5, 0, 1, 255, 137, 153, 61, 29, 0, 0, 0,
	0, 73, 69, 78, 68, 174, 66, 96, 130,
];

const chat = directChatPath(ALICE);
const request = directChatPath(DEMO_CONTACT_REQUESTER);

/** Press the record button and keep it down, as a thumb does. */
function pressAndHoldRecord(): void {
	// A synthetic pointer is not one the browser can capture.
	Element.prototype.setPointerCapture = () => {};
	const record = Array.from(
		document.querySelectorAll<HTMLElement>(
			'[data-testid="message-input-voice-record"]',
		),
	).find(button => button.offsetParent !== null);
	record?.dispatchEvent(
		new PointerEvent('pointerdown', {
			pointerId: 1,
			pointerType: 'touch',
			bubbles: true,
			clientX: 300,
			clientY: 800,
		}),
	);
}

function openFirstPhoto(): void {
	document
		.querySelector<HTMLElement>(
			'[data-testid="message-attachment-photos"] button',
		)
		?.click();
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

export default [
	{ name: 'direct-chat', path: chat },
	{
		name: 'direct-chat-edited-deleted-reacted',
		path: chat,
		seed() {
			const chatId = directChatId(ALICE);
			const mine = chatOperation(
				DEMO_IDS.MY_DEVICE_ID,
				chatId,
				{ type: 'Message', payload: { message: 'See you at 6', media: null } },
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
			const theirs = chatOperation(
				ALICE.deviceId,
				chatId,
				{ type: 'Message', payload: { message: 'Wrong chat', media: null } },
				minutesAgo(2),
			);
			chatOperation(
				ALICE.deviceId,
				chatId,
				{ type: 'DeleteMessage', payload: { hashes: [theirs] } },
				minutesAgo(1),
			);
			const retracted = chatOperation(
				DEMO_IDS.MY_DEVICE_ID,
				chatId,
				{ type: 'Message', payload: { message: 'Never mind', media: null } },
				minutesAgo(1),
			);
			chatOperation(
				DEMO_IDS.MY_DEVICE_ID,
				chatId,
				{ type: 'DeleteMessage', payload: { hashes: [retracted] } },
				minutesAgo(0),
			);
		},
	},
	{
		name: 'direct-chat-blocked',
		path: chat,
		seed() {
			deviceGroupOperation(
				{ type: 'BlockAgent', payload: ALICE.agentId },
				minutesAgo(1),
			);
		},
	},
	{
		name: 'direct-chat-reply-and-long-message',
		path: chat,
		seed() {
			const chatId = directChatId(ALICE);
			const question = chatOperation(
				ALICE.deviceId,
				chatId,
				{
					type: 'Message',
					payload: { message: 'What should I bring?', media: null },
				},
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
		},
	},
	{
		name: 'message-sending',
		path: chat,
		seed() {
			mockBackend.operationsReachMailboxes = false;
			chatOperation(
				DEMO_IDS.MY_DEVICE_ID,
				directChatId(ALICE),
				{
					type: 'Message',
					payload: { message: 'Are you there?', media: null },
				},
				minutesAgo(5),
			);
		},
	},
	{
		name: 'message-unsent',
		path: chat,
		seed() {
			mockBackend.operationsReachMailboxes = false;
			chatOperation(
				DEMO_IDS.MY_DEVICE_ID,
				directChatId(ALICE),
				{
					type: 'Message',
					payload: { message: 'Are you there?', media: null },
				},
				minutesAgo(0),
			);
		},
	},
	{ name: 'contact-request', path: request },
	{
		name: 'accept-contact-request-dialog',
		path: request,
		steps: [{ tap: 'direct-chat-accept-btn' }],
	},
	{
		name: 'security-tips-1',
		path: request,
		steps: [{ tap: 'direct-chat-security-tips' }],
	},
	{
		name: 'security-tips-2',
		path: request,
		steps: [
			{ tap: 'direct-chat-security-tips' },
			{ tap: 'security-tips-next' },
		],
	},
	{
		name: 'security-tips-3',
		path: request,
		steps: [
			{ tap: 'direct-chat-security-tips' },
			{ tap: 'security-tips-next' },
			{ tap: 'security-tips-next' },
		],
	},
	{
		name: 'security-tips-4',
		path: request,
		steps: [
			{ tap: 'direct-chat-security-tips' },
			{ tap: 'security-tips-next' },
			{ tap: 'security-tips-next' },
			{ tap: 'security-tips-next' },
		],
	},
	{
		name: 'security-tips-5',
		path: request,
		steps: [
			{ tap: 'direct-chat-security-tips' },
			{ tap: 'security-tips-next' },
			{ tap: 'security-tips-next' },
			{ tap: 'security-tips-next' },
			{ tap: 'security-tips-next' },
		],
	},
	{ name: 'message-actions', path: chat, steps: [{ hold: 'message-from-me' }] },
	{
		name: 'delete-message-dialog',
		path: chat,
		steps: [{ hold: 'message-from-me' }, { tap: 'message-action-delete' }],
	},
	{ name: 'ios-direct-chat', path: chat, device: 'iphone' },
	{
		name: 'message-editing',
		path: chat,
		steps: [{ hold: 'message-from-me' }, { tap: 'message-action-edit' }],
	},
	{
		name: 'discard-draft-dialog',
		path: chat,
		steps: [
			{ type: 'Draft in progress', into: 'message-input-textarea' },
			{ hold: 'message-from-me' },
			{ tap: 'message-action-edit' },
		],
	},
	{
		name: 'message-replying',
		path: chat,
		steps: [{ hold: 'message-from-me' }, { tap: 'message-action-reply' }],
	},
	{
		name: 'message-reactions',
		path: chat,
		seed() {
			const chatId = directChatId(ALICE);
			const mine = chatOperation(
				DEMO_IDS.MY_DEVICE_ID,
				chatId,
				{ type: 'Message', payload: { message: 'Shall we go?', media: null } },
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
		},
		steps: [{ tap: 'reaction-chip-👍' }],
	},
	{ name: 'direct-chat-media', path: chat, seed: mediaMessages },
	{
		name: 'attach-panel',
		path: chat,
		steps: [{ tap: 'message-input-attach' }],
	},
	{
		name: 'search-results',
		path: `${chat}?search=true`,
		steps: [{ type: 'offline', into: 'direct-chat-search-input' }],
	},
	{
		name: 'search-no-results',
		path: `${chat}?search=true`,
		steps: [{ type: 'zzzz', into: 'direct-chat-search-input' }],
	},
	{
		name: 'direct-chat-reported',
		path: chat,
		seed() {
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
		},
	},
	{ name: 'desktop-direct-chat', path: chat, device: 'desktop' },
	{
		name: 'desktop-message-toolbar',
		path: chat,
		device: 'desktop',
		steps: [{ hover: 'message-from-me' }],
	},
	{
		name: 'photo-lightbox',
		path: chat,
		seed: mediaMessages,
		steps: [{ tap: 'message-attachment-photos' }],
	},
	{
		name: 'error-voice-play',
		path: chat,
		seed: mediaMessages,
		steps: [{ tap: 'voice-play-button' }],
	},
	{
		name: 'staged-photo',
		path: chat,
		steps: [
			() =>
				pasteIntoComposer([
					{ name: 'summit.png', mimeType: 'image/png', bytes: TINY_PNG },
				]),
		],
	},
	{
		name: 'staged-file',
		path: chat,
		steps: [
			() =>
				pasteIntoComposer([
					{ name: 'trail-map.pdf', mimeType: 'application/pdf', size: 2048 },
				]),
		],
	},
	{
		name: 'error-attachment-too-large',
		path: chat,
		steps: [
			() =>
				pasteIntoComposer([
					{
						name: 'film.bin',
						mimeType: 'application/octet-stream',
						size: 17 * 1024 * 1024,
					},
				]),
			() => {
				const sends = document.querySelectorAll<HTMLElement>(
					'[data-testid="message-input-send"]',
				);
				sends[sends.length - 1]?.click();
			},
		],
	},
	{
		name: 'error-one-file-at-a-time',
		path: chat,
		steps: [
			() =>
				pasteIntoComposer([
					{ name: 'a.pdf', mimeType: 'application/pdf', size: 2048 },
					{ name: 'b.pdf', mimeType: 'application/pdf', size: 2048 },
				]),
		],
	},
	{
		name: 'error-files-with-photos',
		path: chat,
		steps: [
			() =>
				pasteIntoComposer([
					{ name: 'a.pdf', mimeType: 'application/pdf', size: 2048 },
					{ name: 'summit.png', mimeType: 'image/png', bytes: TINY_PNG },
				]),
		],
	},
	{
		name: 'error-too-many-attachments',
		path: chat,
		steps: [
			() =>
				pasteIntoComposer(
					Array.from({ length: 40 }, (_, index) => ({
						name: `photo-${index}.png`,
						mimeType: 'image/png',
						bytes: TINY_PNG,
					})),
				),
		],
	},
	{
		name: 'error-damaged-photo',
		path: chat,
		steps: [
			() =>
				pasteIntoComposer([
					{
						name: 'damaged.jpg',
						mimeType: 'image/jpeg',
						bytes: [255, 216, 255],
					},
				]),
		],
	},
	{
		name: 'error-unreadable-photo',
		path: chat,
		steps: [() => window.__test.pasteUnreadablePhoto('withdrawn')],
	},
	{
		name: 'error-unreadable-file',
		path: chat,
		steps: [() => window.__test.pasteUnreadableFile('withdrawn.txt')],
	},
	{
		name: 'error-sending-message',
		path: chat,
		seed() {
			mockBackend.failingCommands = ['send_message'];
		},
		steps: [
			{ type: 'On my way', into: 'message-input-textarea' },
			{ tap: 'message-input-send' },
		],
	},
	{
		name: 'recent-photos-permission',
		path: chat,
		steps: [
			() => {
				window.__test.recentPhotos = { permission: 'prompt', photos: [] };
			},
			{ tap: 'message-input-attach' },
		],
	},
	{
		name: 'ios-recent-photos-no-access',
		path: chat,
		device: 'iphone',
		steps: [
			() => {
				window.__test.recentPhotos = { permission: 'denied', photos: [] };
			},
			{ tap: 'message-input-attach' },
		],
	},
	{
		name: 'direct-chat-yesterday',
		path: chat,
		seed() {
			chatOperation(
				ALICE.deviceId,
				directChatId(BOB),
				{ type: 'Message', payload: { message: 'unused', media: null } },
				minutesAgo(0),
			);
			chatOperation(
				ALICE.deviceId,
				directChatId(ALICE),
				{ type: 'Message', payload: { message: 'Good night!', media: null } },
				minutesAgo(26 * 60),
			);
		},
	},
	{
		name: 'contact-request-with-messages',
		path: request,
		seed() {
			for (const [message, age] of [
				['Hi, we met at the meetup', 9],
				['Is this the right number?', 8],
			] as const) {
				chatOperation(
					DEMO_CONTACT_REQUESTER.deviceId,
					directChatId(DEMO_CONTACT_REQUESTER),
					{ type: 'Message', payload: { message, media: null } },
					minutesAgo(age),
				);
			}
		},
	},
	{
		name: 'direct-chat-scrolled-up',
		path: chat,
		seed() {
			for (let index = 0; index < 40; index++) {
				chatOperation(
					ALICE.deviceId,
					directChatId(ALICE),
					{
						type: 'Message',
						payload: { message: `Trail note ${index + 1}`, media: null },
					},
					minutesAgo(4),
				);
			}
		},
		steps: [
			() => {
				document
					.querySelector('[data-testid="direct-chat-scroll"]')
					?.scrollTo({ top: -100_000 });
			},
		],
	},
	{
		name: 'desktop-photo-lightbox',
		path: chat,
		device: 'desktop',
		seed: mediaMessages,
		steps: [openFirstPhoto],
	},
	{
		name: 'desktop-staged-file',
		path: chat,
		device: 'desktop',
		steps: [
			() =>
				pasteIntoComposer([
					{ name: 'trail-map.pdf', mimeType: 'application/pdf', size: 2048 },
				]),
		],
	},
	{
		name: 'desktop-staged-photos',
		path: chat,
		device: 'desktop',
		steps: [
			() =>
				pasteIntoComposer([
					{ name: 'summit.png', mimeType: 'image/png', bytes: TINY_PNG },
					{ name: 'lake.png', mimeType: 'image/png', bytes: TINY_PNG },
				]),
		],
	},
	{
		name: 'direct-chat-waiting-for-profile',
		path: directChatPath(STRANGER),
		seed() {
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
		},
	},
	{
		name: 'profile-names',
		path: chat,
		steps: [{ tap: 'direct-chat-name-not-verified' }],
	},
	{
		name: 'message-delivered',
		path: chat,
		seed() {
			mockBackend.messageAcks = {
				[DEMO_IDS.MY_DEVICE_ID]: { hash: '', seq: Number.MAX_SAFE_INTEGER },
			};
		},
	},
	{
		name: 'desktop-attach-menu',
		path: chat,
		device: 'desktop',
		steps: [{ tap: 'message-input-attach' }],
	},
	{
		name: 'desktop-drop-files',
		path: chat,
		device: 'desktop',
		steps: [
			() => {
				const dragged = new DataTransfer();
				dragged.items.add(new File(['map'], 'trail-map.pdf'));
				window.dispatchEvent(
					new DragEvent('dragenter', { dataTransfer: dragged, bubbles: true }),
				);
			},
		],
	},
	{
		name: 'desktop-photo-lightbox-second',
		path: chat,
		device: 'desktop',
		seed: mediaMessages,
		steps: [openFirstPhoto, { tap: 'lightbox-next' }],
	},
	{
		name: 'direct-chat-media-missing',
		path: chat,
		seed() {
			mediaMessages();
			mockBackend.mediaIsMissing = true;
		},
	},
	{
		name: 'voice-message-playing',
		path: chat,
		seed: mediaMessages,
		steps: [
			() => {
				HTMLMediaElement.prototype.play = function (this: HTMLMediaElement) {
					this.dispatchEvent(new Event('play'));
					return Promise.resolve();
				};
			},
			{ tap: 'voice-play-button' },
		],
	},
	{
		name: 'recent-photos-denied',
		path: chat,
		steps: [
			() => {
				window.__test.recentPhotos = { permission: 'prompt', photos: [] };
			},
			{ tap: 'message-input-attach' },
			{ tap: 'message-input-recent-photos-allow' },
		],
	},
	{
		name: 'recent-photos-settings',
		path: chat,
		steps: [
			() => {
				window.__test.recentPhotos = { permission: 'prompt', photos: [] };
			},
			{ tap: 'message-input-attach' },
			{ tap: 'message-input-recent-photos-allow' },
			{ tap: 'message-input-recent-photos-allow' },
		],
	},
	{
		name: 'error-downloading-file',
		path: chat,
		seed() {
			mediaMessages();
			mockBackend.mediaIsMissing = true;
		},
		steps: [{ tap: 'message-attachment-file' }],
	},
	{
		name: 'error-loading-recent-photos',
		path: chat,
		steps: [
			() => {
				window.__test.recentPhotos = {
					permission: 'granted',
					get photos(): never {
						throw new Error('the photo library cannot be read');
					},
				};
			},
			{ tap: 'message-input-attach' },
		],
	},
	{
		name: 'error-adding-recent-photo',
		path: chat,
		steps: [
			() => {
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
			},
			{ tap: 'message-input-attach' },
			{ tap: 'message-input-recent-photo-0' },
			untilAToastShows,
		],
	},
	{
		name: 'voice-recording',
		path: chat,
		seed() {
			mockBackend.commandAnswers = {
				'plugin:audio-recorder|check_permission': { granted: true },
			};
		},
		steps: [pressAndHoldRecord],
	},
	{
		name: 'error-microphone-not-allowed',
		path: chat,
		seed() {
			mockBackend.commandAnswers = {
				'plugin:audio-recorder|check_permission': { granted: false },
			};
		},
		steps: [pressAndHoldRecord, untilAToastShows],
	},
	{
		name: 'error-recording-voice',
		path: chat,
		seed() {
			mockBackend.commandAnswers = {
				'plugin:audio-recorder|check_permission': { granted: true },
			};
			mockBackend.failingCommands = ['plugin:audio-recorder|start_recording'];
		},
		steps: [pressAndHoldRecord, untilAToastShows],
	},
	{
		name: 'desktop-file-saved',
		path: chat,
		device: 'desktop',
		seed() {
			mediaMessages();
			mockBackend.commandAnswers = {
				'plugin:dialog|save': '/home/me/trail-map.pdf',
			};
		},
		steps: [
			takeTheAppsOwnPaths,
			{ tap: 'message-attachment-file' },
			untilAToastShows,
		],
	},
	{
		name: 'desktop-photo-saved',
		path: chat,
		device: 'desktop',
		seed() {
			mediaMessages();
			mockBackend.commandAnswers = {
				'plugin:dialog|save': '/home/me/summit.jpg',
			};
		},
		steps: [
			openFirstPhoto,
			takeTheAppsOwnPaths,
			{ tap: 'lightbox-save' },
			untilAToastShows,
		],
	},
] satisfies Scene[];
