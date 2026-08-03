// Environment for spawning `pnpm tauri build` from within the wdio harness.
export function cleanBuildEnv(extra: NodeJS.ProcessEnv): NodeJS.ProcessEnv {
	const base = Object.fromEntries(
		Object.entries(process.env).filter(
			([k]) => k !== 'NODE_OPTIONS' && !/^(npm_|pnpm_)/i.test(k),
		),
	);
	return { ...base, ...extra };
}
