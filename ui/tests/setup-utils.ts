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
function injectVoiceNote(durationMs = 3000, audioDurationMs = durationMs) {
	const wav = buildSilentWav(audioDurationMs);
	const waveform = Array.from({ length: 48 }, (_, i) => 40 + (i % 5) * 40);
	window.dispatchEvent(
		new CustomEvent('test-inject-voice-note', {
			detail: { bytes: Array.from(wav), durationMs, waveform },
		}),
	);
}

/** Pause and seek the first voice note to `fraction` of its real audio length,
 * returning that real fraction (or -1 if the audio isn't loaded yet). Lets specs
 * assert the scrubber maps progress to the audio's own duration, not the
 * (possibly inaccurate) recorded metadata. */
function voiceSeekFraction(fraction: number): number {
	const audio = document.querySelector<HTMLAudioElement>(
		'[data-testid="message-attachment-voice"] audio',
	);
	if (!audio || !isFinite(audio.duration) || audio.duration <= 0) return -1;
	audio.pause();
	audio.currentTime = Math.max(0, Math.min(1, fraction)) * audio.duration;
	return audio.currentTime / audio.duration;
}

/** Read the played fraction (0..1) of the first voice-note waveform from
 * wavesurfer's shadow DOM, so specs can assert playback progress advances. */
function voiceProgress(): number {
	const scrubber = document.querySelector('[data-testid="voice-scrubber"]');
	const progress = scrubber
		?.querySelector('div')
		?.shadowRoot?.querySelector<HTMLElement>('.progress');
	if (!progress) return 0;
	return parseFloat(progress.style.width) / 100 || 0;
}

/** Peak bar luminance of the unplayed (wave) vs played (progress) canvases, so
 * specs can assert the played region is visibly distinct — wavesurfer composites
 * `progressColor` onto the wave canvas with `source-in`, so a translucent
 * waveColor would make the two indistinguishable. */
function voiceBarLuminance(): { unplayed: number; played: number } {
	const shadow = document
		.querySelector('[data-testid="voice-scrubber"]')
		?.querySelector('div')?.shadowRoot;
	const peak = (canvas: HTMLCanvasElement | null | undefined): number => {
		if (!canvas) return 0;
		const ctx = canvas.getContext('2d');
		if (!ctx) return 0;
		const { data } = ctx.getImageData(0, 0, canvas.width, canvas.height);
		let max = 0;
		for (let i = 0; i < data.length; i += 4) {
			const lum =
				((data[i] + data[i + 1] + data[i + 2]) / 3) * (data[i + 3] / 255);
			if (lum > max) max = lum;
		}
		return max;
	};
	return {
		unplayed: peak(shadow?.querySelector('.canvases canvas')),
		played: peak(shadow?.querySelector('.progress canvas')),
	};
}

/** Make the next voice-note byte fetch fail (after `delayMs`, so the play
 * button's loading spinner stays observable), letting specs exercise the
 * load-error toast. Only the `irohblob` blob request is intercepted; the
 * original `fetch` is restored as soon as it fires. */
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

export const testUtils = {
	simulateUpdate,
	hasText,
	pasteFiles,
	dropFiles,
	injectVoiceNote,
	voiceSeekFraction,
	voiceProgress,
	voiceBarLuminance,
	failNextVoiceLoad,
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
