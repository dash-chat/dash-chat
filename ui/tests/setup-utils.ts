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

/** True if the first element matching `selector` contains `text`. */
function hasText(selector: string, text: string): boolean {
	return document.querySelector(selector)?.textContent?.includes(text) ?? false;
}

export const testUtils = {
	simulateUpdate,
	hasText,
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
