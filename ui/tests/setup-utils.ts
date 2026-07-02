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

/** Close this agent's iroh endpoint so it can no longer sync with peers over
 * p2p. Backed by the `disable_p2p` command (only registered under the
 * `e2e-tests` feature). One-way — the agent stays p2p-disconnected until it
 * restarts. */
function disableP2p(): Promise<void> {
	return invokeAfterSetup('disable_p2p');
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

export const testUtils = {
	simulateUpdate,
	hasText,
	disableP2p,
	pasteFiles,
	dropFiles,
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
