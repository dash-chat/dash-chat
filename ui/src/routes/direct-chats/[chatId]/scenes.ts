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
import { open, untilAToastShows } from '$lib/scenes/steps';
import { takeTheAppsOwnPaths } from '$lib/utils/preview-tauri';
import {
	DEMO_CONTACT_REQUESTER,
	DEMO_IDS,
	minutesAgo,
	setMockBackend,
} from 'dash-chat-mock';
import { fill, find, hold, settled, tap } from 'translation-screenshots/scene';

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
	{
		name: 'direct-chat',
		setup: () => open(chat),
	},
	{
		name: 'direct-chat-edited-deleted-reacted',
		async setup() {
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
			await open(chat);
		},
	},
	{
		name: 'direct-chat-blocked',
		async setup() {
			deviceGroupOperation(
				{ type: 'BlockAgent', payload: ALICE.agentId },
				minutesAgo(1),
			);
			await open(chat);
		},
	},
	{
		name: 'direct-chat-reply-and-long-message',
		async setup() {
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
			await open(chat);
		},
	},
	{
		name: 'message-sending',
		async setup() {
			setMockBackend({ operationsReachMailboxes: false });
			chatOperation(
				DEMO_IDS.MY_DEVICE_ID,
				directChatId(ALICE),
				{
					type: 'Message',
					payload: { message: 'Are you there?', media: null },
				},
				minutesAgo(5),
			);
			await open(chat);
		},
	},
	{
		name: 'message-unsent',
		async setup() {
			setMockBackend({ operationsReachMailboxes: false });
			chatOperation(
				DEMO_IDS.MY_DEVICE_ID,
				directChatId(ALICE),
				{
					type: 'Message',
					payload: { message: 'Are you there?', media: null },
				},
				minutesAgo(0),
			);
			await open(chat);
		},
	},
	{
		name: 'contact-request',
		setup: () => open(request),
	},
	{
		name: 'accept-contact-request-dialog',
		async setup() {
			await open(request);
			await tap('direct-chat-accept-btn');
		},
	},
	{
		name: 'security-tips-1',
		async setup() {
			await open(request);
			await tap('direct-chat-security-tips');
		},
	},
	{
		name: 'security-tips-2',
		async setup() {
			await open(request);
			await tap('direct-chat-security-tips');
			await tap('security-tips-next');
		},
	},
	{
		name: 'security-tips-3',
		async setup() {
			await open(request);
			await tap('direct-chat-security-tips');
			await tap('security-tips-next');
			await tap('security-tips-next');
		},
	},
	{
		name: 'security-tips-4',
		async setup() {
			await open(request);
			await tap('direct-chat-security-tips');
			await tap('security-tips-next');
			await tap('security-tips-next');
			await tap('security-tips-next');
		},
	},
	{
		name: 'security-tips-5',
		async setup() {
			await open(request);
			await tap('direct-chat-security-tips');
			await tap('security-tips-next');
			await tap('security-tips-next');
			await tap('security-tips-next');
			await tap('security-tips-next');
		},
	},
	{
		name: 'message-actions',
		async setup() {
			await open(chat);
			await hold('message-from-me');
		},
	},
	{
		name: 'delete-message-dialog',
		async setup() {
			await open(chat);
			await hold('message-from-me');
			await tap('message-action-delete');
		},
	},
	{
		name: 'ios-direct-chat',
		device: 'iphone',
		setup: () => open(chat),
	},
	{
		name: 'message-editing',
		async setup() {
			await open(chat);
			await hold('message-from-me');
			await tap('message-action-edit');
		},
	},
	{
		name: 'discard-draft-dialog',
		async setup() {
			await open(chat);
			await fill('message-input-textarea', 'Draft in progress');
			await hold('message-from-me');
			await tap('message-action-edit');
		},
	},
	{
		name: 'message-replying',
		async setup() {
			await open(chat);
			await hold('message-from-me');
			await tap('message-action-reply');
		},
	},
	{
		name: 'message-reactions',
		async setup() {
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
			await open(chat);
			await tap('reaction-chip-👍');
		},
	},
	{
		name: 'direct-chat-media',
		async setup() {
			mediaMessages();
			await open(chat);
		},
	},
	{
		name: 'attach-panel',
		async setup() {
			await open(chat);
			await tap('message-input-attach');
		},
	},
	{
		name: 'search-results',
		async setup() {
			await open(`${chat}?search=true`);
			await fill('direct-chat-search-input', 'offline');
		},
	},
	{
		name: 'search-no-results',
		async setup() {
			await open(`${chat}?search=true`);
			await fill('direct-chat-search-input', 'zzzz');
		},
	},
	{
		name: 'direct-chat-reported',
		async setup() {
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
		},
	},
	{
		name: 'desktop-direct-chat',
		device: 'desktop',
		setup: () => open(chat),
	},
	{
		name: 'desktop-message-toolbar',
		device: 'desktop',
		async setup() {
			await open(chat);
			const mine = await find('message-from-me');
			// Focus inside the message shows the toolbar a pointer over it does.
			mine
				.querySelector<HTMLElement>('[data-testid="message-hover-react"]')
				?.focus();
		},
	},
	{
		name: 'photo-lightbox',
		async setup() {
			mediaMessages();
			await open(chat);
			openFirstPhoto();
		},
	},
	{
		name: 'error-voice-play',
		async setup() {
			mediaMessages();
			await open(chat);
			await tap('voice-play-button');
		},
	},
	{
		name: 'staged-photo',
		async setup() {
			await open(chat);
			pasteIntoComposer([
				{ name: 'summit.png', mimeType: 'image/png', bytes: TINY_PNG },
			]);
		},
	},
	{
		name: 'staged-file',
		async setup() {
			await open(chat);
			pasteIntoComposer([
				{ name: 'trail-map.pdf', mimeType: 'application/pdf', size: 2048 },
			]);
		},
	},
	{
		name: 'error-attachment-too-large',
		async setup() {
			await open(chat);
			pasteIntoComposer([
				{
					name: 'film.bin',
					mimeType: 'application/octet-stream',
					size: 17 * 1024 * 1024,
				},
			]);
			await settled();
			const sends = document.querySelectorAll<HTMLElement>(
				'[data-testid="message-input-send"]',
			);
			sends[sends.length - 1]?.click();
		},
	},
	{
		name: 'error-one-file-at-a-time',
		async setup() {
			await open(chat);
			pasteIntoComposer([
				{ name: 'a.pdf', mimeType: 'application/pdf', size: 2048 },
				{ name: 'b.pdf', mimeType: 'application/pdf', size: 2048 },
			]);
		},
	},
	{
		name: 'error-files-with-photos',
		async setup() {
			await open(chat);
			pasteIntoComposer([
				{ name: 'a.pdf', mimeType: 'application/pdf', size: 2048 },
				{ name: 'summit.png', mimeType: 'image/png', bytes: TINY_PNG },
			]);
		},
	},
	{
		name: 'error-too-many-attachments',
		async setup() {
			await open(chat);
			pasteIntoComposer(
				Array.from({ length: 40 }, (_, index) => ({
					name: `photo-${index}.png`,
					mimeType: 'image/png',
					bytes: TINY_PNG,
				})),
			);
			await untilAToastShows();
		},
	},
	{
		name: 'error-damaged-photo',
		async setup() {
			await open(chat);
			pasteIntoComposer([
				{
					name: 'damaged.jpg',
					mimeType: 'image/jpeg',
					bytes: [255, 216, 255],
				},
			]);
			await untilAToastShows();
		},
	},
	{
		name: 'error-unreadable-photo',
		async setup() {
			await open(chat);
			window.__test.pasteUnreadablePhoto('withdrawn');
			await untilAToastShows();
		},
	},
	{
		name: 'error-unreadable-file',
		async setup() {
			await open(chat);
			window.__test.pasteUnreadableFile('withdrawn.txt');
			await untilAToastShows();
		},
	},
	{
		name: 'error-sending-message',
		async setup() {
			setMockBackend({ failingCommands: ['send_message'] });
			await open(chat);
			await fill('message-input-textarea', 'On my way');
			await tap('message-input-send');
		},
	},
	{
		name: 'recent-photos-permission',
		async setup() {
			await open(chat);
			window.__test.recentPhotos = { permission: 'prompt', photos: [] };
			await settled();
			await tap('message-input-attach');
		},
	},
	{
		name: 'ios-recent-photos-no-access',
		device: 'iphone',
		async setup() {
			await open(chat);
			window.__test.recentPhotos = { permission: 'denied', photos: [] };
			await settled();
			await tap('message-input-attach');
		},
	},
	{
		name: 'direct-chat-yesterday',
		async setup() {
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
			await open(chat);
		},
	},
	{
		name: 'contact-request-with-messages',
		async setup() {
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
			await open(request);
		},
	},
	{
		name: 'direct-chat-scrolled-up',
		async setup() {
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
			await open(chat);
			document
				.querySelector('[data-testid="direct-chat-scroll"]')
				?.scrollTo({ top: -100_000 });
		},
	},
	{
		name: 'desktop-photo-lightbox',
		device: 'desktop',
		async setup() {
			mediaMessages();
			await open(chat);
			openFirstPhoto();
		},
	},
	{
		name: 'desktop-staged-file',
		device: 'desktop',
		async setup() {
			await open(chat);
			pasteIntoComposer([
				{ name: 'trail-map.pdf', mimeType: 'application/pdf', size: 2048 },
			]);
		},
	},
	{
		name: 'desktop-staged-photos',
		device: 'desktop',
		async setup() {
			await open(chat);
			pasteIntoComposer([
				{ name: 'summit.png', mimeType: 'image/png', bytes: TINY_PNG },
				{ name: 'lake.png', mimeType: 'image/png', bytes: TINY_PNG },
			]);
			await find('message-input-clear-attachments');
		},
	},
	{
		name: 'direct-chat-waiting-for-profile',
		async setup() {
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
		},
	},
	{
		name: 'profile-names',
		async setup() {
			await open(chat);
			await tap('direct-chat-name-not-verified');
		},
	},
	{
		name: 'message-delivered',
		async setup() {
			setMockBackend({
				messageAcks: {
					[DEMO_IDS.MY_DEVICE_ID]: { hash: '', seq: Number.MAX_SAFE_INTEGER },
				},
			});
			await open(chat);
		},
	},
	{
		name: 'desktop-attach-menu',
		device: 'desktop',
		async setup() {
			await open(chat);
			await tap('message-input-attach');
		},
	},
	{
		name: 'desktop-drop-files',
		device: 'desktop',
		async setup() {
			await open(chat);
			const dragged = new DataTransfer();
			dragged.items.add(new File(['map'], 'trail-map.pdf'));
			window.dispatchEvent(
				new DragEvent('dragenter', { dataTransfer: dragged, bubbles: true }),
			);
		},
	},
	{
		name: 'desktop-photo-lightbox-second',
		device: 'desktop',
		async setup() {
			mediaMessages();
			await open(chat);
			openFirstPhoto();
			await settled();
			await tap('lightbox-next');
		},
	},
	{
		name: 'direct-chat-media-missing',
		async setup() {
			mediaMessages();
			setMockBackend({ mediaIsMissing: true });
			await open(chat);
		},
	},
	{
		name: 'voice-message-playing',
		async setup() {
			mediaMessages();
			await open(chat);
			HTMLMediaElement.prototype.play = function (this: HTMLMediaElement) {
				this.dispatchEvent(new Event('play'));
				return Promise.resolve();
			};
			await settled();
			await tap('voice-play-button');
		},
	},
	{
		name: 'recent-photos-denied',
		async setup() {
			await open(chat);
			window.__test.recentPhotos = { permission: 'prompt', photos: [] };
			await settled();
			await tap('message-input-attach');
			await tap('message-input-recent-photos-allow');
		},
	},
	{
		name: 'recent-photos-settings',
		async setup() {
			await open(chat);
			window.__test.recentPhotos = { permission: 'prompt', photos: [] };
			await settled();
			await tap('message-input-attach');
			await tap('message-input-recent-photos-allow');
			await tap('message-input-recent-photos-allow');
		},
	},
	{
		name: 'error-downloading-file',
		async setup() {
			mediaMessages();
			setMockBackend({ mediaIsMissing: true });
			await open(chat);
			await tap('message-attachment-file');
		},
	},
	{
		name: 'error-loading-recent-photos',
		async setup() {
			await open(chat);
			window.__test.recentPhotos = {
				permission: 'granted',
				get photos(): never {
					throw new Error('the photo library cannot be read');
				},
			};
			await settled();
			await tap('message-input-attach');
		},
	},
	{
		name: 'error-adding-recent-photo',
		async setup() {
			await open(chat);
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
			await settled();
			await tap('message-input-attach');
			await tap('message-input-recent-photo-0');
			await untilAToastShows();
		},
	},
	{
		name: 'voice-recording',
		async setup() {
			setMockBackend({
				commandAnswers: {
					'plugin:audio-recorder|check_permission': { granted: true },
				},
			});
			await open(chat);
			pressAndHoldRecord();
		},
	},
	{
		name: 'error-microphone-not-allowed',
		async setup() {
			setMockBackend({
				commandAnswers: {
					'plugin:audio-recorder|check_permission': { granted: false },
				},
			});
			await open(chat);
			pressAndHoldRecord();
			await settled();
			await untilAToastShows();
		},
	},
	{
		name: 'error-recording-voice',
		async setup() {
			setMockBackend({
				commandAnswers: {
					'plugin:audio-recorder|check_permission': { granted: true },
				},
				failingCommands: ['plugin:audio-recorder|start_recording'],
			});
			await open(chat);
			pressAndHoldRecord();
			await settled();
			await untilAToastShows();
		},
	},
	{
		name: 'desktop-file-saved',
		device: 'desktop',
		async setup() {
			mediaMessages();
			setMockBackend({
				commandAnswers: {
					'plugin:dialog|save': '/home/me/trail-map.pdf',
				},
			});
			await open(chat);
			takeTheAppsOwnPaths();
			await settled();
			await tap('message-attachment-file');
			await untilAToastShows();
		},
	},
	{
		name: 'desktop-photo-saved',
		device: 'desktop',
		async setup() {
			mediaMessages();
			setMockBackend({
				commandAnswers: {
					'plugin:dialog|save': '/home/me/summit.jpg',
				},
			});
			await open(chat);
			openFirstPhoto();
			await settled();
			takeTheAppsOwnPaths();
			await settled();
			await tap('lightbox-save');
			await untilAToastShows();
		},
	},
] satisfies Scene[];
