const STORAGE_KEY = 'preview-features-enabled';

let enabled = $state(
	typeof localStorage !== 'undefined'
		? localStorage.getItem(STORAGE_KEY) === 'true'
		: false,
);

export const previewFeatures = {
	get enabled() {
		return enabled;
	},
	enable() {
		enabled = true;
		localStorage.setItem(STORAGE_KEY, 'true');
	},
	toggle() {
		enabled = !enabled;
		localStorage.setItem(STORAGE_KEY, String(enabled));
	},
};
