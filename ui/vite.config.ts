import { paraglideVitePlugin } from '@inlang/paraglide-js';
import { sveltekit } from '@sveltejs/kit/vite';
import tailwindcss from '@tailwindcss/vite';
import { createRequire } from 'node:module';
import path from 'node:path';
import { defineConfig } from 'vite';

// Resolve signalium's development ESM build for production builds
const require = createRequire(import.meta.url);
const signaliumPkgDir = path.dirname(require.resolve('signalium/package.json'));
const signaliumDevIndex = path.join(
	signaliumPkgDir,
	'dist/esm/development/index.js',
);

// Set by Tauri's mobile `--host` dev (it injects this into the beforeDevCommand
// it spawns Vite with). The LAN IP a physical device uses to reach both the dev
// server and the HMR websocket. Unset on desktop, where it stays on localhost.
const host = process.env.TAURI_DEV_HOST;
const uiPort = parseInt(process.env.UI_PORT || '1420', 10);

// https://vite.dev/config/
export default defineConfig(async () => ({
	optimizeDeps: {
		exclude: ['dash-chat-stores'],
		// Pre-include dash-chat-stores' transitive deps so Vite doesn't discover
		// them at runtime and re-optimize, which causes duplicate module instances
		include: ['blakejs', 'emittery'],
	},
	resolve: {
		dedupe: ['svelte', 'svelte/internal', 'svelte/internal/client'],
		alias: [
			{
				find: /^signalium$/,
				replacement: signaliumDevIndex,
			},
			{
				find: /^signalium\/(.+)$/,
				replacement: path.join(signaliumPkgDir, 'dist/esm/development/$1.js'),
			},
		],
	},
	plugins: [
		sveltekit(),
		tailwindcss(),
		paraglideVitePlugin({
			project: './project.inlang',
			outdir: './src/lib/paraglide',
			strategy: ['preferredLanguage', 'cookie', 'globalVariable', 'baseLocale'],
		}),
	],
	// Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
	//
	// 1. prevent Vite from obscuring rust errors
	clearScreen: false,
	// 2. tauri expects a fixed port; fail fast if it is already taken
	server: {
		port: uiPort,
		strictPort: true,
		host: true,
		hmr: host ? { protocol: 'ws', host, port: uiPort } : undefined,
		fs: { allow: ['tests', 'src', 'node_modules', '.svelte-kit', '..'] },
		watch: {
			// 3. tell Vite to ignore watching `src-tauri`
			ignored: ['**/src-tauri/**'],
		},
	},
}));
