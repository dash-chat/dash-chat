import { isTauriEnv } from './environment';

/**
 * Forward `console.*` from the WebView through `@tauri-apps/plugin-log`
 * so JS logs land in the same stream as Rust logs.
 *
 * Requires the Rust side to allow the "webview" log target at the desired
 * level (see `src-tauri/src/setup.rs`).
 */
export function forwardConsoleToTauriLog(): void {
	if (!isTauriEnv()) return;

	import('@tauri-apps/plugin-log').then(({ debug, info, warn, error }) => {
		const orig = {
			log: console.log,
			info: console.info,
			warn: console.warn,
			error: console.error,
			debug: console.debug,
		};

		const fmt = (args: unknown[]) =>
			args
				.map(a => {
					if (typeof a === 'string') return a;
					try {
						return JSON.stringify(a);
					} catch {
						return String(a);
					}
				})
				.join(' ');

		console.log = (...args) => {
			orig.log(...args);
			info(fmt(args));
		};
		console.info = (...args) => {
			orig.info(...args);
			info(fmt(args));
		};
		console.warn = (...args) => {
			orig.warn(...args);
			warn(fmt(args));
		};
		console.error = (...args) => {
			orig.error(...args);
			error(fmt(args));
		};
		console.debug = (...args) => {
			orig.debug(...args);
			debug(fmt(args));
		};
	});
}
