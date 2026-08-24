const STORAGE_KEY = 'developer-mode-unlocked';

const initialUnlocked =
	typeof localStorage !== 'undefined'
		? localStorage.getItem(STORAGE_KEY) === 'true'
		: false;

let unlocked = $state(initialUnlocked);

export const developerMode = {
	get unlocked() {
		return unlocked;
	},
	unlock() {
		unlocked = true;
		localStorage.setItem(STORAGE_KEY, 'true');
	},
	lock() {
		unlocked = false;
		localStorage.removeItem(STORAGE_KEY);
	},
};
