import { isMobile, isTauriEnv } from '$lib/utils/environment';
import { overrideSystemBarsColorScheme } from 'tauri-plugin-system-theme';

/**
 * Force light (white) system bars, for the always-dark overlays (lightbox,
 * staged-media page) whose background is dark regardless of the app theme.
 */
export async function setLightSystemBars() {
	if (!isMobile || !isTauriEnv()) return;
	await overrideSystemBarsColorScheme('light');
}

/** Give the system bars back to the app's colour scheme. */
export async function applyThemeSystemBars() {
	if (!isMobile || !isTauriEnv()) return;
	await overrideSystemBarsColorScheme(null);
}

export function applyDarkMode(dark: boolean) {
	document.documentElement.classList.toggle('dark', dark);
	document.documentElement.classList.toggle('wa-dark', dark);
	document.documentElement.style.colorScheme = dark ? 'dark' : 'light';
}
