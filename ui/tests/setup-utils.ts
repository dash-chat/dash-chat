/**
 * Registers browser-side test utilities on `window.__test`.
 *
 * Only keep helpers here that genuinely need to execute inside the page:
 *   - app-bound helpers (`tr`/`goto`/`setLocale`)
 *   - browser-event helpers (`simulateUpdate`, `hasText`)
 *
 * Single-purpose DOM queries belong in `e2e-tests/helpers/pages/*`.
 */
import { invokeAfterSetup } from 'dash-chat-stores';

import type { m } from '../src/lib/paraglide/messages.js';

type Messages = typeof m;

/** Fake recent-photos data the composer strip reads via `window.__test`. This is
 * the single source of truth for the seam's shape — `$lib/utils/recent-photos`
 * infers it from the `window.__test` declaration below. Kept here (not imported
 * from the `$lib`-aliased prod module) so the e2e tsconfig, which compiles this
 * file for the global augmentation, doesn't have to resolve `$lib`. */
export interface RecentPhotosTestData {
	permission: 'granted' | 'denied' | 'prompt';
	photos: { id: string; name: string; mimeType: string; dataUrl: string }[];
}
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

const mediaDownloads = new Map<string, number>();
let mediaObserver: PerformanceObserver | undefined;

/** Start recording how long each blob request takes. An observer rather than a
 * read of `performance.getEntriesByType` at the end: the resource-timing buffer
 * holds a few hundred entries and a media-heavy chat overruns it. */
function recordMediaDownloads() {
	if (mediaObserver !== undefined) return;
	mediaObserver = new PerformanceObserver(list => {
		for (const entry of list.getEntries()) {
			// First request only: a blob rendered on a second surface (gallery cell,
			// filmstrip thumb, lightbox stage) re-requests the same URL and is served
			// from cache, which would otherwise overwrite the real download with ~0.
			if (!mediaDownloads.has(entry.name)) {
				mediaDownloads.set(entry.name, entry.duration);
			}
		}
	});
	mediaObserver.observe({ type: 'resource', buffered: true });
}

/** Milliseconds the photo whose alt contains `label` spent downloading — the
 * webview issuing the `irohblob://` request to its last byte — or null while it
 * is still in flight. Needs `recordMediaDownloads` to have run first. */
function photoDownloadMs(label: string): number | null {
	const img = Array.from(document.querySelectorAll('img')).find(i =>
		i.alt.includes(label),
	);
	if (img === undefined) return null;
	const ms = mediaDownloads.get(img.src);
	return ms === undefined ? null : Math.round(ms);
}

/** Close this agent's iroh endpoint so it can no longer sync with peers over
 * p2p. Backed by the `close_iroh_endpoint` command (only registered under the
 * `e2e-tests` feature). One-way — the agent stays p2p-disconnected until it
 * restarts. */
function disableP2p(): Promise<void> {
	return invokeAfterSetup('close_iroh_endpoint');
}

/** Reset the app to first-launch state: clear web storage, then run the real
 * `delete_account` command — the same code path as Settings → Account →
 * Delete account — which shuts the node down, deletes the data dir, and (on
 * mobile) exits the app. The iOS e2e harness calls this before each spec
 * instead of reinstalling the .ipa, which is the only other way iOS can reset
 * app data. Fire-and-forget: on mobile the command exits the app, so its
 * invoke never resolves. */
function resetToFirstLaunch(): void {
	localStorage.clear();
	sessionStorage.clear();
	void invokeAfterSetup('delete_account');
}

/** Summon the Android soft keyboard for the currently focused input. A
 * WebDriver click focuses the input but does not reliably raise the IME, so
 * keyboard-behavior specs summon it natively, the way the app itself does. */
function showKeyboard(): Promise<void> {
	return invokeAfterSetup('plugin:virtual-keyboard|show');
}
export interface TestFileSpec {
	name: string;
	mimeType: string;
	/** Raw bytes. Omit and pass `size` for a large zero-filled file so a huge
	 * array doesn't have to cross the WebDriver bridge. */
	bytes?: number[];
	size?: number;
}

/** xorshift32 over `buf`. Deterministic per seed, and incompressible enough
 * that a measured payload size is the size that actually crosses the wire. */
function fillPseudoRandom(buf: Uint8Array | Uint8ClampedArray, seed: number) {
	let x = seed >>> 0 || 1;
	for (let i = 0; i < buf.length; i++) {
		x ^= x << 13;
		x >>>= 0;
		x ^= x >>> 17;
		x ^= x << 5;
		x >>>= 0;
		buf[i] = x & 0xff;
	}
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

function specsToFiles(specs: TestFileSpec[]): File[] {
	return specs.map(spec => {
		const data = spec.bytes
			? new Uint8Array(spec.bytes)
			: new Uint8Array(spec.size ?? 0);
		return new File([data], spec.name, { type: spec.mimeType });
	});
}

function dispatchPaste(files: File[]) {
	const textarea = document.querySelector(
		'[data-testid="message-input-textarea"]',
	);
	if (!textarea) throw new Error('Composer textarea not found');
	const event = new ClipboardEvent('paste', {
		bubbles: true,
		cancelable: true,
	});
	// A synthetic DataTransfer reports an empty `.files` in WKWebView (iOS), so the
	// composer's paste handler (which reads `clipboardData.files`) sees nothing.
	// Expose a plain FileList-like that every engine reads correctly.
	Object.defineProperty(event, 'clipboardData', { value: { files } });
	textarea.dispatchEvent(event);
}

export interface NoisePhotoSpec {
	name: string;
	width: number;
	height: number;
	quality?: number;
}

/**
 * Paste a synthesized noise JPEG of the given pixel size, resolving with the
 * bytes it encoded to. Noise rather than flat colour so JPEG can't compress the
 * payload away and every call yields a distinct blob — a repeated identical
 * photo would resolve from the receiver's blob store and measure nothing.
 */
async function pasteNoisePhoto(spec: NoisePhotoSpec): Promise<number> {
	const canvas = document.createElement('canvas');
	canvas.width = spec.width;
	canvas.height = spec.height;
	const ctx = canvas.getContext('2d');
	if (!ctx) throw new Error('canvas context failed');
	const image = ctx.createImageData(spec.width, spec.height);
	// Seeded by name as well as the clock: on the clock alone, two attaches
	// landing in the same millisecond encode to identical bytes and collapse into
	// one blob.
	let seed = Date.now() & 0xffffffff;
	for (let i = 0; i < spec.name.length; i++) {
		seed = (seed * 31 + spec.name.charCodeAt(i)) | 0;
	}
	fillPseudoRandom(image.data, seed);
	for (let i = 3; i < image.data.length; i += 4) image.data[i] = 255;
	ctx.putImageData(image, 0, 0);
	const blob = await new Promise<Blob>((resolve, reject) =>
		canvas.toBlob(
			b => (b ? resolve(b) : reject(new Error('canvas.toBlob failed'))),
			'image/jpeg',
			spec.quality ?? 0.9,
		),
	);
	dispatchPaste([new File([blob], spec.name, { type: 'image/jpeg' })]);
	return blob.size;
}

/**
 * Dispatch a synthetic paste of the given files onto the composer textarea.
 * WebKit drops constructor-init clipboardData, so it is attached via
 * defineProperty.
 */
function pasteFiles(specs: TestFileSpec[]) {
	dispatchPaste(specsToFiles(specs));
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

/** Bypasses the native recorder (no microphone in the WebKitGTK harness); the
 * composer listens for `test-inject-voice-note`. */
function injectVoiceNote(durationMs = 3000, audioDurationMs = durationMs) {
	const wav = buildSilentWav(audioDurationMs);
	const waveform = Array.from({ length: 48 }, (_, i) => 40 + (i % 5) * 40);
	window.dispatchEvent(
		new CustomEvent('test-inject-voice-note', {
			detail: { bytes: Array.from(wav), durationMs, waveform },
		}),
	);
}

/** Seeks to `fraction` of the real audio length, so specs can assert the
 * scrubber follows the audio rather than the recorded metadata. */
function voiceSeekFraction(fraction: number): number {
	const audio = document.querySelector<HTMLAudioElement>(
		'[data-testid="message-attachment-voice"] audio',
	);
	if (!audio || !isFinite(audio.duration) || audio.duration <= 0) return -1;
	audio.pause();
	audio.currentTime = Math.max(0, Math.min(1, fraction)) * audio.duration;
	return audio.currentTime / audio.duration;
}

function voiceProgress(): number {
	const played = document.querySelector<HTMLElement>(
		'[data-testid="voice-scrubber-played"]',
	);
	if (!played) return 0;
	return parseFloat(played.style.width) / 100 || 0;
}

/** On-screen luminance of an unplayed vs played waveform bar (the bar's color
 * alpha-composited over the bubble background), to prove the played region is
 * visibly distinct. */
function voiceBarLuminance(): { unplayed: number; played: number } {
	const scrubber = document.querySelector<HTMLElement>(
		'[data-testid="voice-scrubber"]',
	);
	if (!scrubber) return { unplayed: 0, played: 0 };
	const channels = (color: string): number[] =>
		(color.match(/\d+(\.\d+)?/g) ?? []).map(Number);
	let background = [255, 255, 255];
	for (
		let el: HTMLElement | null = scrubber;
		el !== null;
		el = el.parentElement
	) {
		const parts = channels(getComputedStyle(el).backgroundColor);
		if (parts.length >= 3 && (parts.length < 4 || parts[3] > 0)) {
			background = parts;
			break;
		}
	}
	const backgroundLum = (background[0] + background[1] + background[2]) / 3;
	const composite = (span: Element | null): number => {
		if (!span) return 0;
		const style = getComputedStyle(span);
		const [r = 0, g = 0, b = 0] = channels(style.color);
		const alpha = parseFloat(style.opacity);
		return ((r + g + b) / 3) * alpha + backgroundLum * (1 - alpha);
	};
	return {
		unplayed: composite(scrubber.querySelector(':scope > div > span')),
		played: composite(
			scrubber.querySelector('[data-testid="voice-scrubber-played"] span'),
		),
	};
}

/** Fails the next voice-note byte fetch after `delayMs` so the spinner stays
 * observable. Only `irohblob` is intercepted, and `fetch` is restored at once. */
function failNextVoiceLoad(delayMs = 0) {
	const original = window.fetch;
	window.fetch = (input: RequestInfo | URL, init?: RequestInit) => {
		const url =
			typeof input === 'string'
				? input
				: input instanceof URL
					? input.href
					: input.url;
		if (!url.includes('irohblob')) return original(input, init);
		window.fetch = original;
		return new Promise<Response>((_, reject) =>
			setTimeout(
				() => reject(new Error('test: forced voice load failure')),
				delayMs,
			),
		);
	};
}

/** What a file input the app opened was configured to ask the OS for. */
export interface FilePickerRequest {
	accept: string;
	/** `false` when the input asks for a stored file rather than a fresh capture. */
	capture: boolean;
	multiple: boolean;
}

const nativeInputClick = HTMLInputElement.prototype.click;
let filePickerRequests: FilePickerRequest[] = [];

/**
 * Record the file inputs the app opens instead of letting them reach the OS,
 * answering each with `files` — or, when there are none, backing out of it as
 * if the user had dismissed the dialog.
 */
function interceptFilePickers(files: TestFileSpec[] = []): void {
	filePickerRequests = [];
	HTMLInputElement.prototype.click = function (this: HTMLInputElement) {
		if (this.type !== 'file') {
			nativeInputClick.call(this);
			return;
		}
		filePickerRequests.push({
			accept: this.accept,
			capture: this.hasAttribute('capture'),
			multiple: this.multiple,
		});
		if (files.length === 0) {
			this.dispatchEvent(new Event('cancel'));
			return;
		}
		Object.defineProperty(this, 'files', {
			value: specsToDataTransfer(files).files,
			configurable: true,
		});
		this.dispatchEvent(new Event('change'));
	};
}

/** Restore file inputs and return what `interceptFilePickers` recorded. */
function collectFilePickers(): FilePickerRequest[] {
	HTMLInputElement.prototype.click = nativeInputClick;
	return filePickerRequests;
}

export const testUtils = {
	simulateUpdate,
	hasText,
	disableP2p,
	resetToFirstLaunch,
	showKeyboard,
	pasteFiles,
	pasteNoisePhoto,
	dropFiles,
	injectVoiceNote,
	voiceSeekFraction,
	voiceProgress,
	voiceBarLuminance,
	failNextVoiceLoad,
	recordMediaDownloads,
	photoDownloadMs,
	interceptFilePickers,
	collectFilePickers,
	/** E2E override for the composer's recent-photos strip; left undefined unless
	 * a spec injects fake photos (the native library is unavailable in tests). */
	recentPhotos: undefined as RecentPhotosTestData | undefined,
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
	/** Dispatch a deep link URL through the app's full routing logic. */
	handleDeepLink: (_url: string): void => {
		throw new Error(
			'handleDeepLink called before registerTestUtils provided the callback',
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
	handleDeepLink?: (url: string) => void,
) {
	window.__test = testUtils;
	if (enablePreviewFeatures) {
		testUtils.enablePreviewFeatures = enablePreviewFeatures;
	}
	if (handleDeepLink) {
		testUtils.handleDeepLink = handleDeepLink;
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
