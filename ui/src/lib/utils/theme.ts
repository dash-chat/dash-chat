import { isMobile, isTauriEnv } from '$lib/utils/environment';

type BarStyle = 'light' | 'dark';

export async function setSystemBarsStyle(
	statusBarStyle: BarStyle,
	navigationBarStyle: BarStyle,
) {
	if (!isMobile || !isTauriEnv()) return;
	const { invoke } = await import('@tauri-apps/api/core');
	await invoke('plugin:system-bars-styles|set_style', {
		statusBarStyle,
		navigationBarStyle,
		navigationBarTransparent: true,
	});
}

/** Restore the system-bar style to match the currently-applied theme. */
export async function applyThemeSystemBars() {
	const dark = document.documentElement.classList.contains('dark');
	await setSystemBarsStyle(dark ? 'light' : 'dark', dark ? 'light' : 'dark');
}

export async function applyDarkMode(dark: boolean) {
	document.documentElement.classList.toggle('dark', dark);
	document.documentElement.classList.toggle('wa-dark', dark);
	await applyThemeSystemBars();
}
