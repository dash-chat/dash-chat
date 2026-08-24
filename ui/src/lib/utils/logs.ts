import { m } from '$lib/paraglide/messages.js';
import { showToast } from '$lib/utils/toasts';
import { debug, error, info, warn } from '@tauri-apps/plugin-log';

import { isTauriEnv } from './environment';
import { isAppShuttingDown } from './shutdown';

let installed = false;

/**
 * Forward `console.*` from the WebView through `@tauri-apps/plugin-log`
 * so JS logs land in the same stream as Rust logs.
 *
 * Requires the Rust side to allow the "webview" log target at the desired
 * level (see `src-tauri/src/setup.rs`).
 */
export function forwardConsoleToTauriLog(): void {
	if (installed || !isTauriEnv()) return;
	installed = true;

	const orig = {
		log: console.log,
		info: console.info,
		warn: console.warn,
		error: console.error,
		debug: console.debug,
	};

	const fmtOne = (a: unknown): string => {
		if (typeof a === 'string') return a;
		if (a instanceof Error) return a.stack ?? `${a.name}: ${a.message}`;
		try {
			return JSON.stringify(a);
		} catch {
			return String(a);
		}
	};

	const fmt = (args: unknown[]) => args.map(fmtOne).join(' ');

	const ignoredWarnings = [
		// emoji-picker-element fires this on every load because the Tauri
		// asset protocol doesn't set an ETag. Caching still works via
		// IndexedDB; the warning is purely a freshness-check hint.
		'emoji-picker-element is more efficient if the dataSource server exposes an ETag header.',
		"Couldn't find callback id",
	];
	const isIgnoredWarning = (args: unknown[]) =>
		typeof args[0] === 'string' &&
		ignoredWarnings.some(w => (args[0] as string).includes(w));

	// Swallow IPC rejections from the log plugin. Otherwise a failed call from
	// console.error → error() would unhandle-reject back into console.error
	// and recurse.
	console.log = (...args) => {
		orig.log(...args);
		info(fmt(args)).catch(() => {});
	};
	console.info = (...args) => {
		orig.info(...args);
		info(fmt(args)).catch(() => {});
	};
	console.warn = (...args) => {
		if (isIgnoredWarning(args)) return;
		orig.warn(...args);
		warn(fmt(args)).catch(() => {});
	};
	console.error = (...args) => {
		orig.error(...args);
		error(fmt(args)).catch(() => {});
	};
	console.debug = (...args) => {
		orig.debug(...args);
		debug(fmt(args)).catch(() => {});
	};
}

let errorHandlersInstalled = false;

const describe = (value: unknown): string =>
	value instanceof Error
		? (value.stack ?? `${value.name}: ${value.message}`)
		: String(value);

// Browsers surface a ResizeObserver delivery loop as an uncaught error. It is a
// benign scheduling notice the spec requires, not an app failure, and every
// layout pass can raise one.
const ignoredErrors = ['ResizeObserver loop'];

const isIgnoredError = (message: string) =>
	ignoredErrors.some(ignored => message.includes(ignored));

/**
 * Log uncaught exceptions and unhandled rejections, and surface them as an
 * unexpected-error toast.
 */
export function reportUncaughtErrors(): void {
	if (errorHandlersInstalled) return;
	errorHandlersInstalled = true;

	window.addEventListener('error', event => {
		if (isAppShuttingDown() || isIgnoredError(event.message)) return;
		const where = event.filename
			? ` @ ${event.filename}:${event.lineno}:${event.colno}`
			: '';
		console.error(
			`[uncaught] ${event.message}${where}\n${describe(event.error)}`,
		);
		showToast(m.errorUnexpected(), 'unexpected', event.error);
	});
	window.addEventListener('unhandledrejection', event => {
		if (isAppShuttingDown()) return;
		console.error(`[unhandledrejection] ${describe(event.reason)}`);
		showToast(m.errorUnexpected(), 'unexpected', event.reason);
	});
}
