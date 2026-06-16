import { m } from '$lib/paraglide/messages.js';
import { loadMediaBytes } from '$lib/types/media';
import { isMobile, isTauriEnv } from '$lib/utils/environment';
import { showToast } from '$lib/utils/toasts';
import type { FileAttachment, Photo } from 'dash-chat-stores';

function isShareCancelled(error: unknown): boolean {
	const message =
		error instanceof Error
			? error.message
			: typeof error === 'string'
				? error
				: '';
	return message.trim() === 'Share cancelled';
}

/**
 * Mobile has no native save dialog, so write the bytes to the app cache and
 * hand them to the system share sheet (which offers Save to Files / Save
 * Image). Same pattern as `shareQrCode`.
 */
async function shareAttachmentOnMobile(
	file: FileAttachment | Photo,
): Promise<void> {
	const [{ shareFile }, { appCacheDir, join }, { mkdir, writeFile }] =
		await Promise.all([
			import('@choochmeque/tauri-plugin-sharekit-api'),
			import('@tauri-apps/api/path'),
			import('@tauri-apps/plugin-fs'),
		]);
	const shareDir = await join(await appCacheDir(), 'share');
	await mkdir(shareDir, { recursive: true });
	// The attachment name comes from a peer; keep only a basename so it can
	// never escape the share directory.
	const name = file.name.split(/[\\/]/).pop() || 'attachment';
	const path = await join(shareDir, name);
	await writeFile(path, await loadMediaBytes(file));
	try {
		await shareFile(`file://${path}`, {
			mimeType: file.mime_type,
			title: name,
		});
	} catch (error) {
		if (isShareCancelled(error)) return;
		throw error;
	}
}

/**
 * Save an attachment: native save dialog (defaulting to the downloads
 * folder) on desktop Tauri, system share sheet on mobile, anchor-download
 * fallback in the browser. Toasts on desktop success and on unexpected
 * failure.
 */
export async function saveAttachment(
	file: FileAttachment | Photo,
): Promise<void> {
	try {
		if (isTauriEnv() && isMobile) {
			await shareAttachmentOnMobile(file);
		} else if (isTauriEnv()) {
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
			await writeFile(path, await loadMediaBytes(file));
			showToast(m.fileSaved());
		} else {
			const bytes = await loadMediaBytes(file);
			const url = URL.createObjectURL(
				new Blob([bytes], { type: file.mime_type }),
			);
			const a = document.createElement('a');
			a.href = url;
			a.download = file.name;
			a.click();
			// Revoking synchronously can race the download start.
			setTimeout(() => URL.revokeObjectURL(url), 0);
		}
	} catch (e) {
		showToast(m.errorUnexpected(), 'unexpected', e);
	}
}
