const STORAGE_KEY = 'developer-features-enabled';

let enabled = $state(
	typeof localStorage !== 'undefined'
		? localStorage.getItem(STORAGE_KEY) === 'true'
		: false,
);

export const developerFeatures = {
	get enabled() {
		return enabled;
	},
	toggle() {
		enabled = !enabled;
		localStorage.setItem(STORAGE_KEY, String(enabled));
	},
};
