/**
 * Environment for spawning `pnpm tauri build` from within the wdio harness.
 *
 * wdio runs under a TypeScript loader it exposes via `NODE_OPTIONS`; inherited
 * by the child `pnpm` (itself a node script), the loader intercepts pnpm's own
 * imports and makes it abort resolving a phantom `.pnpmfile.mjs` (a pnpm quirk
 * with `"type": "module"`). Stripping `NODE_OPTIONS` and pnpm's injected
 * `npm_*`/`pnpm_*` vars lets pnpm run as it does from a plain shell — the same
 * way `just ios build` / `just tauri build` invoke it directly. `extra` is
 * merged on top (e.g. MAILBOX_URL, VITE_E2E, CARGO_* overrides).
 */
export function cleanBuildEnv(extra: NodeJS.ProcessEnv): NodeJS.ProcessEnv {
	const base = Object.fromEntries(
		Object.entries(process.env).filter(
			([k]) => k !== 'NODE_OPTIONS' && !/^(npm_|pnpm_)/i.test(k),
		),
	);
	return { ...base, ...extra };
}
