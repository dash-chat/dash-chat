import { m } from '$lib/paraglide/messages.js';
import { isMobile, isTauriEnv } from '$lib/utils/environment';
import { shareFile } from '@choochmeque/tauri-plugin-sharekit-api';
import { appCacheDir, downloadDir, join } from '@tauri-apps/api/path';
import { save } from '@tauri-apps/plugin-dialog';
import { mkdir, writeFile } from '@tauri-apps/plugin-fs';
import type { FileAttachment, Photo } from 'dash-chat-stores';

/**
 * Write raw bytes to disk: native save dialog (defaulting to the downloads
 * folder) on desktop Tauri, anchor-download fallback in the browser. Returns
 * `true` when the file was written via the desktop dialog (so the caller can
 * confirm with a toast), and `false` for the browser download or when the
 * user cancelled the dialog. Throws on unexpected failure.
 *
 * `title` is shown in the desktop save dialog; `filters` are the extension
 * filters it offers.
 */
export async function saveFile(
	data: Uint8Array,
	fileName: string,
	mimeType: string,
	title: string,
	filters?: { name: string; extensions: string[] }[],
): Promise<boolean> {
	if (isTauriEnv()) {
		let defaultPath = fileName;
		try {
			defaultPath = await join(await downloadDir(), fileName);
		} catch {
			// downloadDir may not exist on some platforms; fall back to bare name
		}
		const path = await save({ title, defaultPath, filters });
		if (!path) return false;
		await writeFile(path, data);
		return true;
	}
	const url = URL.createObjectURL(
		new Blob([new Uint8Array(data)], { type: mimeType }),
	);
	const a = document.createElement('a');
	a.href = url;
	a.download = fileName;
	a.click();
	// Revoking synchronously can race the download start.
	setTimeout(() => URL.revokeObjectURL(url), 0);
	return false;
}

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
	const shareDir = await join(await appCacheDir(), 'share');
	await mkdir(shareDir, { recursive: true });
	// The attachment name comes from a peer; keep only a basename so it can
	// never escape the share directory.
	const name = file.name.split(/[\\/]/).pop() || 'attachment';
	const path = await join(shareDir, name);
	await writeFile(path, file.data);
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
 * Save an attachment: native save dialog on desktop, system share sheet
 * on mobile, anchor-download fallback in the browser. Returns `true` when the
 * file was written to disk via the desktop dialog (so the caller can confirm
 * with a toast), and `false` otherwise. Throws on unexpected failure.
 */
export async function saveAttachment(
	file: FileAttachment | Photo,
): Promise<boolean> {
	if (isTauriEnv() && isMobile) {
		await shareAttachmentOnMobile(file);
		return false;
	}
	return saveFile(file.data, file.name, file.mime_type, m.saveFile());
}
