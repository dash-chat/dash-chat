import { failIfTold, mockBackend } from 'dash-chat-stores';

const MISSING_MEDIA = '/missing-media.bin';

const PLACEHOLDER_PHOTO = `data:image/svg+xml,${encodeURIComponent(
	'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 400 300">' +
		'<defs><linearGradient id="sky" x2="0" y2="1"><stop offset="0" stop-color="#8fb8e8"/><stop offset="1" stop-color="#e8d9b8"/></linearGradient></defs>' +
		'<rect width="400" height="300" fill="url(#sky)"/>' +
		'<path d="M0 300 L120 130 L200 220 L280 100 L400 300 Z" fill="#5d7a5a"/>' +
		'<circle cx="320" cy="70" r="28" fill="#fff6d8"/></svg>',
)}`;

/** Have the preview take the paths the app itself takes, for what the app
 * only does there: a native save dialog, say. Its answers still come from the
 * stand-in. */
export function takeTheAppsOwnPaths(): void {
	// eslint-disable-next-line
	(window as any).__TAURI_INTERNALS__.preview = false;
}

/** Answer the UI's direct calls to Tauri — plugins, asset URLs — in the
 * browser preview, where there is no Tauri: left unanswered, each one throws
 * wherever the UI happens to make it. Data comes from the mock clients; this
 * only keeps what goes around them from breaking the page. */
export function installPreviewTauri(): void {
	// eslint-disable-next-line
	(window as any).__TAURI_INTERNALS__ = {
		preview: true,
		invoke: async (command: string) => {
			failIfTold(command);
			if (command in mockBackend.commandAnswers) {
				return mockBackend.commandAnswers[command];
			}
			return command === 'plugin:app|version' ? 'preview' : undefined;
		},
		convertFileSrc: () =>
			mockBackend.mediaIsMissing ? MISSING_MEDIA : PLACEHOLDER_PHOTO,
		transformCallback: () => 0,
	};
}
