import { isMobile, isTauriEnv } from '$lib/utils/environment';

type BarStyle = 'light' | 'dark';

async function setSystemBarsStyle(
	statusBarStyle: BarStyle,
	navigationBarStyle: BarStyle,
) {
	const { invoke } = await import('@tauri-apps/api/core');
	await invoke('plugin:system-bars-styles|set_style', {
		statusBarStyle,
		navigationBarStyle,
		navigationBarTransparent: true,
	});
}

export async function applyDarkMode(dark: boolean) {
	document.documentElement.classList.toggle('dark', dark);
	document.documentElement.classList.toggle('wa-dark', dark);
	if (isMobile && isTauriEnv()) {
		await setSystemBarsStyle(dark ? 'light' : 'dark', dark ? 'light' : 'dark');
	}
}
