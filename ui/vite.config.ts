import { paraglideVitePlugin } from '@inlang/paraglide-js';
import { sveltekit } from '@sveltejs/kit/vite';
import tailwindcss from '@tailwindcss/vite';
// @ts-ignore no type declarations
import localIpAddress from 'local-ip-address';
import { createRequire } from 'node:module';
import path from 'node:path';
import { defineConfig } from 'vite';

// Resolve signalium's development ESM build for production builds
const require = createRequire(import.meta.url);
const signaliumPkgDir = path.dirname(
	require.resolve('signalium/package.json'),
);
const signaliumDevIndex = path.join(
	signaliumPkgDir,
	'dist/esm/development/index.js',
);

// const host = process.env.TAURI_DEV_HOST;
const host = localIpAddress();
const uiPort = parseInt(process.env.UI_PORT || '1420', 10);

// https://vite.dev/config/
export default defineConfig(async () => ({
	optimizeDeps: {
		exclude: ['dash-chat-stores'],
		// Pre-include dash-chat-stores' transitive deps so Vite doesn't discover
		// them at runtime and re-optimize, which causes duplicate module instances
		include: ['base64-js', 'cbor-web', 'blakejs', 'emittery'],
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
		tailwindcss(),
		sveltekit(),
		paraglideVitePlugin({
			project: './project.inlang',
			outdir: './src/lib/paraglide',
		}),
		// If the desired port was already taken, another Vite is serving — exit cleanly.
		{
			name: 'exit-if-port-taken',
			configureServer(server: any) {
				server.httpServer?.on('listening', () => {
					const addr = server.httpServer?.address();
					if (addr && typeof addr === 'object' && addr.port !== uiPort) {
						console.log(`Port ${uiPort} already in use, skipping Vite startup`);
						process.exit(0);
					}
				});
			},
		},
	],
	// Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
	//
	// 1. prevent Vite from obscuring rust errors
	clearScreen: false,
	// 2. tauri expects a fixed port; the exit-if-port-taken plugin silently exits if already in use
	server: {
		port: uiPort,
		host: true,
		hmr: host ? { protocol: 'ws', host, port: uiPort + 1 } : undefined,
		fs: { allow: ['tests', 'src', 'node_modules', '.svelte-kit', '..'] },
		watch: {
			// 3. tell Vite to ignore watching `src-tauri`
			ignored: ['**/src-tauri/**'],
		},
	},
}));
