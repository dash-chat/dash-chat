import { isTauriEnv } from '$lib/utils/environment';

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
