import { invoke } from '@tauri-apps/api/core';
import { isMobile } from '$lib/utils/environment';

type BarStyle = 'light' | 'dark';

async function setSystemBarsStyle(statusBarStyle: BarStyle, navigationBarStyle: BarStyle) {
	await invoke('plugin:system-bars-styles|set_style', {
		statusBarStyle,
		navigationBarStyle,
	});
}

export async function applyDarkMode(dark: boolean) {
	document.documentElement.classList.toggle('dark', dark);
	document.documentElement.classList.toggle('wa-dark', dark);
	if (isMobile) {
		await setSystemBarsStyle(dark ? 'light' : 'dark', dark ? 'light' : 'dark');
	}
}
