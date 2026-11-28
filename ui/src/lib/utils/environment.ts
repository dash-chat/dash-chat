export function isTauriEnv() {
	const internals = window.__TAURI_INTERNALS__;
	// The browser preview installs a stand-in (preview-tauri.ts).
	return internals !== undefined && internals.preview !== true;
}

export const isIos = /iPhone|iPad|iPod/i.test(navigator.userAgent);
export const isMac = /Macintosh|Mac OS X/i.test(navigator.userAgent);
export const isAndroid = /Android/i.test(navigator.userAgent);

export const isMobile = isIos || isAndroid;
