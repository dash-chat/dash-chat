import { emit } from '@tauri-apps/api/event';
import { mockIPC } from '@tauri-apps/api/mocks';
import { backend, seedDemoData } from 'dash-chat-mock';

/** Give the browser preview, where there is no Tauri, a fake backend to talk
 * to: the one in this page, on the demo data, unless whoever drives the page
 * holds one and has put it at `__fakeBackend`. */
export function installPreviewTauri(): void {
	window.__TAURI_INTERNALS__ = {
		preview: true,
		convertFileSrc: () => '/preview-photo.svg',
	};
	const elsewhere = window.__fakeBackend;
	if (elsewhere === undefined) {
		seedDemoData();
		backend.outbox = {
			// Copies, as what a real backend sends is: the page must not end up
			// holding the backend's own logs.
			event: (name, payload) => void emit(name, structuredClone(payload)),
			stream: (channel, index, message) =>
				window.__TAURI_INTERNALS__?.runCallback?.(channel, { index, message }),
		};
	}
	mockIPC(
		(command, args) => {
			// As they cross to a real backend: channels become their ids.
			const sent = JSON.parse(JSON.stringify(args ?? {})) as Record<
				string,
				unknown
			>;
			return elsewhere === undefined
				? structuredClone(backend.handle(command, sent))
				: elsewhere(command, sent);
		},
		{ shouldMockEvents: true },
	);
}
