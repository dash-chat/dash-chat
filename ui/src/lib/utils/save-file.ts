import { m } from '$lib/paraglide/messages.js';
import { asUint8Array, bytesToBlobUrl } from '$lib/types/media';
import { isTauriEnv } from '$lib/utils/environment';
import { showToast } from '$lib/utils/toasts';
import type { FileAttachment, Photo } from 'dash-chat-stores';

/**
 * Save an attachment to disk: native save dialog (defaulting to the
 * downloads folder) under Tauri, anchor-download fallback in the browser.
 * Toasts on success and on unexpected failure.
 */
export async function saveAttachment(
	file: FileAttachment | Photo,
): Promise<void> {
	try {
		if (isTauriEnv()) {
			const [{ save }, { writeFile }, { downloadDir, join }] =
				await Promise.all([
					import('@tauri-apps/plugin-dialog'),
					import('@tauri-apps/plugin-fs'),
					import('@tauri-apps/api/path'),
				]);
			let defaultPath = file.name;
			try {
				defaultPath = await join(await downloadDir(), file.name);
			} catch {
				// downloadDir may not exist on some platforms; fall back to bare name
			}
			const path = await save({ title: m.saveFile(), defaultPath });
			if (!path) return;
			await writeFile(path, asUint8Array(file.data));
			showToast(m.fileSaved());
		} else {
			const url = bytesToBlobUrl(file.data, file.mime_type);
			const a = document.createElement('a');
			a.href = url;
			a.download = file.name;
			a.click();
			URL.revokeObjectURL(url);
		}
	} catch (e) {
		showToast(m.errorUnexpected(), 'unexpected', e);
	}
}
