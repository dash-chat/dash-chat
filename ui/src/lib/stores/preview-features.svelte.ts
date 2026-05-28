import { writable } from 'svelte/store';

const STORAGE_KEY = 'preview-features-enabled';

const initialEnabled =
	typeof localStorage !== 'undefined'
		? localStorage.getItem(STORAGE_KEY) === 'true'
		: false;

let enabled = $state(initialEnabled);

export const previewFeaturesEnabled = writable(initialEnabled);

export const previewFeatures = {
	get enabled() {
		return enabled;
	},
	enable() {
		enabled = true;
		previewFeaturesEnabled.set(true);
		localStorage.setItem(STORAGE_KEY, 'true');
	},
	toggle() {
		enabled = !enabled;
		previewFeaturesEnabled.set(enabled);
		localStorage.setItem(STORAGE_KEY, String(enabled));
	},
};
