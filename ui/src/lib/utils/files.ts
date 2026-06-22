import { isTauriEnv } from '$lib/utils/environment';
import { shareFile as shareFileNative } from '@choochmeque/tauri-plugin-sharekit-api';
import { appCacheDir, basename, join } from '@tauri-apps/api/path';
import { open, save } from '@tauri-apps/plugin-dialog';
import { mkdir, readFile, writeFile } from '@tauri-apps/plugin-fs';

/**
 * Write raw bytes to disk: native save dialog on desktop Tauri, anchor-download
 * fallback in the browser. Returns `true` when the file was written via the
 * desktop dialog (so the caller can confirm with a toast), and `false` for the
 * browser download or when the user cancelled the dialog. Throws on unexpected
 * failure.
 *
 * `title` is shown in the desktop save dialog and `folder` is its default
 * directory; `filters` are the extension filters it offers.
 */
export async function saveFile(
	data: Uint8Array,
	folder: string,
	fileName: string,
	mimeType: string,
	title: string,
	filters?: { name: string; extensions: string[] }[],
): Promise<boolean> {
	if (isTauriEnv()) {
		let defaultPath = fileName;
		if (folder) {
			try {
				defaultPath = await join(folder, fileName);
			} catch {
				// folder may not exist on some platforms; fall back to bare name
			}
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
 * Write bytes to the app cache and hand them to the system share sheet (which
 * offers Save to Files / Save Image). Used where there is no native save
 * dialog.
 */
export async function shareFile(
	data: Uint8Array,
	fileName: string,
	mimeType: string,
): Promise<void> {
	const shareDir = await join(await appCacheDir(), 'share');
	await mkdir(shareDir, { recursive: true });
	// The attachment name comes from a peer; keep only a basename so it can
	// never escape the share directory.
	const name = fileName.split(/[\\/]/).pop() || 'attachment';
	const path = await join(shareDir, name);
	await writeFile(path, data);
	try {
		await shareFileNative(`file://${path}`, { mimeType, title: name });
	} catch (error) {
		if (isShareCancelled(error)) return;
		throw error;
	}
}

const EXTENSION_MIME: Record<string, string> = {
	jpg: 'image/jpeg',
	jpeg: 'image/jpeg',
	png: 'image/png',
	webp: 'image/webp',
	gif: 'image/gif',
};

function mimeFromName(name: string): string {
	const ext = name.split('.').pop()?.toLowerCase() ?? '';
	return EXTENSION_MIME[ext] ?? 'application/octet-stream';
}

async function pathToFile(path: string): Promise<File> {
	const bytes = await readFile(path);
	const name = await basename(path);
	return new File([bytes], name, { type: mimeFromName(name) });
}

/**
 * Open a native mobile picker via tauri-plugin-dialog and resolve with the
 * chosen files, or `null` if dismissed. `'image'` mode opens the system photo
 * library (PHPickerViewController on iOS) directly; `'document'` mode opens the
 * file browser directly — skipping the action sheet a web `<input type=file>`
 * would show. The plugin copies each pick into the app cache, so we read the
 * bytes back to rebuild a `File` (the MIME type is inferred from the name since
 * the picker only returns paths).
 */
export async function pickNativeFiles(opts: {
	mode: 'image' | 'document';
	multiple: boolean;
}): Promise<File[] | null> {
	const selection = await open({
		multiple: opts.multiple,
		pickerMode: opts.mode,
	});
	const paths =
		selection === null
			? []
			: Array.isArray(selection)
				? selection
				: [selection];
	if (paths.length === 0) return null;
	return Promise.all(paths.map(pathToFile));
}

/**
 * Open the native file picker and resolve with the chosen files, or `null` if
 * the dialog was dismissed. The `<input>` is appended (some webviews require it
 * to be in the document for `click()`) and removed once the pick settles.
 */
export function pickFiles(opts: {
	accept?: string;
	multiple?: boolean;
}): Promise<FileList | null> {
	return new Promise(resolve => {
		const input = document.createElement('input');
		input.type = 'file';
		if (opts.accept) input.accept = opts.accept;
		input.multiple = opts.multiple ?? false;
		input.style.display = 'none';

		const finish = (files: FileList | null) => {
			input.remove();
			resolve(files);
		};
		input.addEventListener('change', () => finish(input.files), { once: true });
		input.addEventListener('cancel', () => finish(null), { once: true });

		document.body.appendChild(input);
		input.click();
	});
}
