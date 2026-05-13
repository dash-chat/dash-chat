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
