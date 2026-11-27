import type { Scene } from '$lib/scenes';
import {
	ALICE,
	BOB,
	STRANGER,
	TINY_PNG_BYTES,
	directChatId,
	directChatPath,
	pasteIntoComposer,
} from '$lib/scenes/demo-world';
import { hold, open, tid, untilAToastShows } from '$lib/scenes/steps';
import { takeTheAppsOwnPaths } from '$lib/utils/preview-tauri';
import {
	DEMO_CONTACT_REQUESTER,
	DEMO_IDS,
	chatOperation,
	deviceGroupOperation,
	minutesAgo,
	setMockBackend,
	textMessage,
} from 'dash-chat-mock';
import { fill, find, present, settled, tap } from 'i18n-screenshots/scene';

const chat = directChatPath(ALICE);
const request = directChatPath(DEMO_CONTACT_REQUESTER);

/** Press the record button and keep it down, as a thumb does. */
async function pressAndHoldRecord(): Promise<void> {
	// A synthetic pointer is not one the browser can capture.
	Element.prototype.setPointerCapture = () => {};
	const record = await find('message-input-voice-record');
	record.dispatchEvent(
		new PointerEvent('pointerdown', {
			pointerId: 1,
			pointerType: 'touch',
			bubbles: true,
			clientX: 300,
			clientY: 800,
		}),
	);
}

async function openFirstPhoto(): Promise<void> {
	const firstPhoto = await present(
		`${tid('message-attachment-photos')} button`,
	);
	firstPhoto.click();
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

/** The security tips of a contact request, turned to the `tip`th of them. */
function securityTip(tip: number): Scene {
	return {
		name: `security-tips-${tip}`,
		async setup() {
			await open(request);
			await tap('direct-chat-security-tips');
			for (let turned = 1; turned < tip; turned++) {
				await tap('security-tips-next');
			}
		},
	};
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
		},
	},
	{
		name: 'message-sending',
		async setup() {
			setMockBackend({ operationsReachMailboxes: false });
			textMessage(
				DEMO_IDS.MY_DEVICE_ID,
				directChatId(ALICE),
				'Are you there?',
				minutesAgo(5),
			);
			await open(chat);
		},
	},
	{
		name: 'message-unsent',
		async setup() {
			setMockBackend({ operationsReachMailboxes: false });
			textMessage(
				DEMO_IDS.MY_DEVICE_ID,
				directChatId(ALICE),
				'Are you there?',
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
	...[1, 2, 3, 4, 5].map(securityTip),
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
			// Focus inside the message shows the toolbar a pointer over it does.
			const react = await present(
				`${tid('message-from-me')} ${tid('message-hover-react')}`,
			);
			react.focus();
		},
	},
	{
		name: 'photo-lightbox',
		async setup() {
			mediaMessages();
			await open(chat);
			await openFirstPhoto();
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
			await pasteIntoComposer([
				{ name: 'summit.png', mimeType: 'image/png', bytes: TINY_PNG_BYTES },
			]);
		},
	},
	{
		name: 'staged-file',
		async setup() {
			await open(chat);
			await pasteIntoComposer([
				{ name: 'trail-map.pdf', mimeType: 'application/pdf', size: 2048 },
			]);
		},
	},
	{
		name: 'error-attachment-too-large',
		async setup() {
			await open(chat);
			await pasteIntoComposer([
				{
					name: 'film.bin',
					mimeType: 'application/octet-stream',
					size: 17 * 1024 * 1024,
				},
			]);
			// The composer's own send button is under the page the file is staged on.
			const send = await present(
				`${tid('staged-media-page')} ${tid('message-input-send')}`,
			);
			send.click();
			await untilAToastShows();
		},
	},
	{
		name: 'error-one-file-at-a-time',
		async setup() {
			await open(chat);
			await pasteIntoComposer([
				{ name: 'a.pdf', mimeType: 'application/pdf', size: 2048 },
				{ name: 'b.pdf', mimeType: 'application/pdf', size: 2048 },
			]);
		},
	},
	{
		name: 'error-files-with-photos',
		async setup() {
			await open(chat);
			await pasteIntoComposer([
				{ name: 'a.pdf', mimeType: 'application/pdf', size: 2048 },
				{ name: 'summit.png', mimeType: 'image/png', bytes: TINY_PNG_BYTES },
			]);
		},
	},
	{
		name: 'error-too-many-attachments',
		async setup() {
			await open(chat);
			await pasteIntoComposer(
				Array.from({ length: 40 }, (_, index) => ({
					name: `photo-${index}.png`,
					mimeType: 'image/png',
					bytes: TINY_PNG_BYTES,
				})),
			);
			await untilAToastShows();
		},
	},
	{
		name: 'error-damaged-photo',
		async setup() {
			await open(chat);
			await pasteIntoComposer([
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
			await find('message-input-textarea');
			window.__test.pasteUnreadablePhoto('withdrawn');
			await untilAToastShows();
		},
	},
	{
		name: 'error-unreadable-file',
		async setup() {
			await open(chat);
			await find('message-input-textarea');
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
			textMessage(ALICE.deviceId, directChatId(BOB), 'unused', minutesAgo(0));
			textMessage(
				ALICE.deviceId,
				directChatId(ALICE),
				'Good night!',
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
				textMessage(
					DEMO_CONTACT_REQUESTER.deviceId,
					directChatId(DEMO_CONTACT_REQUESTER),
					message,
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
				textMessage(
					ALICE.deviceId,
					directChatId(ALICE),
					`Trail note ${index + 1}`,
					minutesAgo(4),
				);
			}
			await open(chat);
			(await find('direct-chat-scroll')).scrollTo({ top: -100_000 });
		},
	},
	{
		name: 'desktop-photo-lightbox',
		device: 'desktop',
		async setup() {
			mediaMessages();
			await open(chat);
			await openFirstPhoto();
		},
	},
	{
		name: 'desktop-staged-file',
		device: 'desktop',
		async setup() {
			await open(chat);
			await pasteIntoComposer([
				{ name: 'trail-map.pdf', mimeType: 'application/pdf', size: 2048 },
			]);
		},
	},
	{
		name: 'desktop-staged-photos',
		device: 'desktop',
		async setup() {
			await open(chat);
			await pasteIntoComposer([
				{ name: 'summit.png', mimeType: 'image/png', bytes: TINY_PNG_BYTES },
				{ name: 'lake.png', mimeType: 'image/png', bytes: TINY_PNG_BYTES },
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
			window.__test.dragFilesOver([
				{ name: 'trail-map.pdf', mimeType: 'application/pdf', size: 2048 },
			]);
		},
	},
	{
		name: 'desktop-photo-lightbox-second',
		device: 'desktop',
		async setup() {
			mediaMessages();
			await open(chat);
			await openFirstPhoto();
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
			await pressAndHoldRecord();
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
			await pressAndHoldRecord();
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
			await pressAndHoldRecord();
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
			await openFirstPhoto();
			await settled();
			takeTheAppsOwnPaths();
			await settled();
			await tap('lightbox-save');
			await untilAToastShows();
		},
	},
] satisfies Scene[];
