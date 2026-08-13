import { isTauriEnv } from '$lib/utils/environment';
import { openUrl } from '@tauri-apps/plugin-opener';

/** Opens a url in the system's default browser. */
export async function openExternalUrl(url: string): Promise<void> {
	if (isTauriEnv()) {
		await openUrl(url);
	} else {
		window.open(url, '_blank', 'noopener');
	}
}
