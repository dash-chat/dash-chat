/**
 * Registers browser-side test utilities on `window.__test`.
 *
 * Only keep helpers here that genuinely need to execute inside the page:
 *   - app-bound helpers (`tr`/`goto`/`setLocale`)
 *   - browser-event helpers (`simulateUpdate`, `hasText`)
 *
 * Single-purpose DOM queries belong in `e2e-tests/helpers/pages/*`.
 */
import type { m } from '../src/lib/paraglide/messages.js';

type Messages = typeof m;
type MessageKey = Extract<keyof Messages, string>;
type MessageParams<K extends MessageKey> = Parameters<Messages[K]>[0];

/** Trigger UpdaterBanner into a specific state via custom event. */
function simulateUpdate(
	state: 'available' | 'downloading' | 'ready' | 'error' | 'hidden',
) {
	window.dispatchEvent(
		new CustomEvent('test-simulate-update', { detail: state }),
	);
}

/** Force any BlobImage whose alt matches into its error/retry state. */
function forceBlobError(alt: string) {
	window.dispatchEvent(
		new CustomEvent('test-blob-force-error', { detail: alt }),
	);
}

/** True if the first element matching `selector` contains `text`. */
function hasText(selector: string, text: string): boolean {
	return document.querySelector(selector)?.textContent?.includes(text) ?? false;
}

export interface TestFileSpec {
	name: string;
	mimeType: string;
	/** Raw bytes. Omit and pass `size` for a large zero-filled file so a huge
	 * array doesn't have to cross the WebDriver bridge. */
	bytes?: number[];
	size?: number;
}

function specsToDataTransfer(specs: TestFileSpec[]): DataTransfer {
	const dt = new DataTransfer();
	for (const spec of specs) {
		const data = spec.bytes
			? new Uint8Array(spec.bytes)
			: new Uint8Array(spec.size ?? 0);
		dt.items.add(new File([data], spec.name, { type: spec.mimeType }));
	}
	return dt;
}

/**
 * Dispatch a synthetic paste of the given files onto the composer textarea.
 * WebKit drops constructor-init clipboardData, so it is attached via
 * defineProperty.
 */
function pasteFiles(specs: TestFileSpec[]) {
	const textarea = document.querySelector(
		'[data-testid="message-input-textarea"]',
	);
	if (!textarea) throw new Error('Composer textarea not found');
	const event = new ClipboardEvent('paste', {
		bubbles: true,
		cancelable: true,
	});
	Object.defineProperty(event, 'clipboardData', {
		value: specsToDataTransfer(specs),
	});
	textarea.dispatchEvent(event);
}

/**
 * Dispatch a synthetic drop of the given files on the window, exercising the
 * MediaDropOverlay HTML5 pipeline. See `pasteFiles` for the defineProperty
 * rationale.
 */
function dropFiles(specs: TestFileSpec[]) {
	const dt = specsToDataTransfer(specs);
	for (const type of ['dragenter', 'drop'] as const) {
		const event = new DragEvent(type, { bubbles: true, cancelable: true });
		Object.defineProperty(event, 'dataTransfer', { value: dt });
		window.dispatchEvent(event);
	}
}

/** Build a valid silent 16 kHz mono 16-bit WAV of `durationMs` for tests. */
function buildSilentWav(durationMs: number): Uint8Array {
	const sampleRate = 16000;
	const numSamples = Math.max(1, Math.floor((sampleRate * durationMs) / 1000));
	const dataSize = numSamples * 2;
	const buffer = new ArrayBuffer(44 + dataSize);
	const view = new DataView(buffer);
	const writeStr = (offset: number, s: string) => {
		for (let i = 0; i < s.length; i++)
			view.setUint8(offset + i, s.charCodeAt(i));
	};
	writeStr(0, 'RIFF');
	view.setUint32(4, 36 + dataSize, true);
	writeStr(8, 'WAVE');
	writeStr(12, 'fmt ');
	view.setUint32(16, 16, true);
	view.setUint16(20, 1, true); // PCM
	view.setUint16(22, 1, true); // mono
	view.setUint32(24, sampleRate, true);
	view.setUint32(28, sampleRate * 2, true); // byte rate
	view.setUint16(32, 2, true); // block align
	view.setUint16(34, 16, true); // bits per sample
	writeStr(36, 'data');
	view.setUint32(40, dataSize, true);
	return new Uint8Array(buffer);
}

/**
 * Stage a synthetic voice note in the composer, bypassing the native recorder
 * (microphone capture is unavailable in the WebKitGTK test harness). The
 * composer listens for `test-inject-voice-note` and sets a voice draft.
 */
function injectVoiceNote(durationMs = 3000) {
	const wav = buildSilentWav(durationMs);
	const waveform = Array.from({ length: 48 }, (_, i) => 40 + (i % 5) * 40);
	window.dispatchEvent(
		new CustomEvent('test-inject-voice-note', {
			detail: { bytes: Array.from(wav), durationMs, waveform },
		}),
	);
}

export const testUtils = {
	simulateUpdate,
	hasText,
	pasteFiles,
	dropFiles,
	injectVoiceNote,
	forceBlobError,
	/** Resolve a paraglide message in the current locale (set by registerTestUtils). */
	tr<K extends MessageKey>(key: K, _params?: MessageParams<K>): string {
		throw new Error(
			`tr(${JSON.stringify(key)}) called before registerTestUtils provided messages`,
		);
	},
	/** Paraglide setLocale — set by registerTestUtils from +layout.svelte. */
	setLocale: (_locale: string) => {},
	/** SvelteKit goto — set by registerTestUtils from +layout.svelte. */
	goto: (_path: string) => Promise.resolve() as Promise<void>,
	/** Enable preview features — set by registerTestUtils from +layout.svelte. */
	enablePreviewFeatures: (): void => {
		throw new Error(
			'enablePreviewFeatures called before registerTestUtils provided the callback',
		);
	},
};

declare global {
	interface Window {
		__test: typeof testUtils;
	}
}

export function registerTestUtils(
	goto?: (path: string) => Promise<void>,
	setLocale?: (locale: string) => void,
	messages?: Messages,
	enablePreviewFeatures?: () => void,
) {
	window.__test = testUtils;
	if (enablePreviewFeatures) {
		testUtils.enablePreviewFeatures = enablePreviewFeatures;
	}
	if (goto) {
		testUtils.goto = goto;
	}
	if (setLocale) {
		testUtils.setLocale = setLocale;
	}
	if (messages) {
		testUtils.tr = <K extends MessageKey>(
			key: K,
			params?: MessageParams<K>,
		): string => {
			const message = messages[key] as
				| ((inputs: MessageParams<K>) => string)
				| undefined;
			if (!message) {
				throw new Error(`tr: missing paraglide message for key "${key}"`);
			}
			const value = message((params ?? {}) as MessageParams<K>);
			if (!value) {
				throw new Error(`tr: paraglide message for key "${key}" is empty`);
			}
			return value;
		};
	}
}
