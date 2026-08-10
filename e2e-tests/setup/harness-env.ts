// Environment for processes the harness spawns that are not wdio spec workers.
// wdio exports its tsx loader through NODE_OPTIONS so its workers inherit it, and
// pnpm exports its own settings the same way; anything else spawned picks them up
// as collateral, which makes nested pnpm calls fail.
export function envWithoutWdioLoader(
	extra: NodeJS.ProcessEnv = {},
): NodeJS.ProcessEnv {
	const base = Object.fromEntries(
		Object.entries(process.env).filter(
			([k]) => k !== 'NODE_OPTIONS' && !/^(npm_|pnpm_)/i.test(k),
		),
	);
	return { ...base, ...extra };
}
