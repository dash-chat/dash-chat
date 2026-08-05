import { isMobile, isTauriEnv } from '$lib/utils/environment';

type BarStyle = 'light' | 'dark';

async function setSystemBarsStyle(
	statusBarStyle: BarStyle,
	navigationBarStyle: BarStyle,
) {
	const { invokeAfterSetup } = await import('dash-chat-stores');
	await invokeAfterSetup('plugin:system-bars-styles|set_style', {
		statusBarStyle,
		navigationBarStyle,
		navigationBarTransparent: true,
	});
}

/**
 * Force light (white) system bars, for the always-dark overlays (lightbox,
 * staged-media page) whose background is dark regardless of the app theme.
 */
export async function setLightSystemBars() {
	if (isMobile && isTauriEnv()) {
		await setSystemBarsStyle('light', 'light');
	}
}

/** Restore the system-bar style to match the currently-applied theme. */
export async function applyThemeSystemBars() {
	if (!isMobile || !isTauriEnv()) return;
	const dark = document.documentElement.classList.contains('dark');
	await setSystemBarsStyle(dark ? 'light' : 'dark', dark ? 'light' : 'dark');
}

export async function applyDarkMode(dark: boolean) {
	document.documentElement.classList.toggle('dark', dark);
	document.documentElement.classList.toggle('wa-dark', dark);
	document.documentElement.style.colorScheme = dark ? 'dark' : 'light';
	await applyThemeSystemBars();
}
