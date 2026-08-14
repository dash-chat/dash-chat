import { writable } from 'svelte/store';

const STORAGE_KEY = 'offline-mode-enabled';

const initialEnabled =
	typeof localStorage !== 'undefined'
		? localStorage.getItem(STORAGE_KEY) === 'true'
		: false;

let enabled = $state(initialEnabled);

export const offlineModeEnabled = writable(initialEnabled);

export const offlineMode = {
	get enabled() {
		return enabled;
	},
	toggle() {
		enabled = !enabled;
		offlineModeEnabled.set(enabled);
		localStorage.setItem(STORAGE_KEY, String(enabled));
	},
};
