// Tauri doesn't have a Node.js server to do proper SSR
// so we use adapter-static with a fallback to index.html to put the site in SPA mode
// See: https://svelte.dev/docs/kit/single-page-apps
// See: https://v2.tauri.app/start/frontend/sveltekit/ for more info
import adapter from '@sveltejs/adapter-static';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';
import { readFileSync } from 'node:fs';

const tauriConf = JSON.parse(
	readFileSync(
		new URL('../src-tauri/tauri.conf.json', import.meta.url),
		'utf8',
	),
);

/** @type {import('@sveltejs/kit').Config} */
const config = {
	preprocess: vitePreprocess(),
	kit: {
		adapter: adapter({
			fallback: 'index.html',
		}),
		// The default version is Date.now(), which changes every chunk hash per build.
		version: { name: tauriConf.version },
		alias: {
			$messages: 'messages',
		},
	},
};

export default config;
