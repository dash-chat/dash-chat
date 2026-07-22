import { m } from '$lib/paraglide/messages.js';
import { isTauriEnv } from '$lib/utils/environment';
import { showToast } from '$lib/utils/toasts';

export async function copyLinkToClipboard(link: string): Promise<void> {
	try {
		await writeText(link);
		showToast(m.copiedLinkToClipboard());
	} catch (e) {
		console.error(e);
		showToast(m.errorUnexpected(), 'unexpected', e);
	}
}

export async function writeText(text: string): Promise<void> {
	if (isTauriEnv()) {
		const { writeText: tauriWriteText } = await import(
			'@tauri-apps/plugin-clipboard-manager'
		);
		await tauriWriteText(text);
	} else {
		await navigator.clipboard.writeText(text);
	}
}
