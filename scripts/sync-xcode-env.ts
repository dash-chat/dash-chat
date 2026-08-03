/**
 * Persist build-time vars into Xcode's `.xcode.env.local`. Xcode compiles the
 * Rust in its own environment (not the caller's shell), so the `.env.${ENV}`
 * vars won't reach the `option_env!` reads baked in at compile time. The "Build
 * Rust Code" phase sources `.xcode.env.local`, so persist them there.
 */
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const ENV_LOCAL = path.join(
	ROOT,
	'src-tauri',
	'gen',
	'apple',
	'.xcode.env.local',
);

/** Keys the "Build Rust Code" phase must see at compile time (`option_env!`). */
export const MANAGED = ['MAILBOX_URL', 'PUSH_NOTIFICATIONS_SERVER_URL'] as const;
export type ManagedKey = (typeof MANAGED)[number];

/**
 * Rewrite `.xcode.env.local` so the managed keys reflect `vars`: prior lines for
 * the managed keys are dropped, then the ones present (non-empty) in `vars` are
 * re-added. Unmanaged lines (e.g. manual PATH overrides) are left untouched.
 */
export function syncXcodeEnv(vars: Partial<Record<ManagedKey, string>>): void {
	const existing = existsSync(ENV_LOCAL)
		? readFileSync(ENV_LOCAL, 'utf8').split('\n')
		: [];
	const kept = existing.filter(
		line =>
			line.trim() !== '' && !MANAGED.some(k => line.startsWith(`export ${k}=`)),
	);
	const added = MANAGED.filter(k => vars[k]).map(
		k => `export ${k}="${vars[k]}"`,
	);
	writeFileSync(ENV_LOCAL, `${[...kept, ...added].join('\n')}\n`);
}
