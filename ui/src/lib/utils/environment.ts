export function isTauriEnv() {
	// eslint-disable-next-line
	return !!(window as any).__TAURI_INTERNALS__;
}

export const isIos = /iPhone|iPad|iPod/i.test(navigator.userAgent);
export const isAndroid = /Android/i.test(navigator.userAgent);

export const isMobile = isIos || isAndroid;
