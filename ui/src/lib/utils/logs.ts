import { debug, error, info, warn } from '@tauri-apps/plugin-log';

import { isTauriEnv } from './environment';

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

	// The engine prints these through an internal channel that bypasses our
	// patched `console.*`, so they never reach the Tauri log otherwise.
	window.addEventListener('unhandledrejection', event => {
		error(`Unhandled promise rejection: ${fmtOne(event.reason)}`).catch(
			() => {},
		);
	});
	window.addEventListener('error', event => {
		const detail = event.error
			? fmtOne(event.error)
			: `${event.message} (${event.filename}:${event.lineno}:${event.colno})`;
		error(`Uncaught error: ${detail}`).catch(() => {});
	});
}
