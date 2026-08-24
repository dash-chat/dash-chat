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

/**
 * Build inputs the "Build Rust Code" phase must see. The `*_URL` vars are read
 * by `option_env!`; the `*_PORT` vars `just dev` allocates are read by
 * `build.rs`, which synthesizes a LAN-reachable URL from them.
 */
export const MANAGED = [
	'MAILBOX_URL',
	'PUSH_NOTIFICATIONS_SERVER_URL',
	'MAILBOX_PORT',
	'PUSH_NOTIFICATIONS_SERVER_PORT',
] as const;
export type ManagedKey = (typeof MANAGED)[number];

/**
 * Persist build-time vars into Xcode's `.xcode.env.local`. Xcode compiles the
 * Rust in its own environment (not the caller's shell), so neither the
 * `.env.${ENV}` vars nor the ports `just dev` exports reach the build at all.
 * The "Build Rust Code" phase sources `.xcode.env.local`, so persist them there.
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
