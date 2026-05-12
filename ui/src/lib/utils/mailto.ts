import { invoke } from '@tauri-apps/api/core';
import { appCacheDir, join } from '@tauri-apps/api/path';
import { mkdir, writeFile } from '@tauri-apps/plugin-fs';

interface MailtoRequest {
	subject: string;
	body: string;
	includeDebugLog: boolean;
	attachments?: File[];
}

export async function sendMailto(request: MailtoRequest): Promise<void> {
	let attachments: string[] | undefined;
	let body = request.body;
	if (request.includeDebugLog) {
		try {
			const redactedPath = await invoke<string>('get_redacted_log');
			attachments = [redactedPath];
		} catch (e) {
			console.error('Failed to get redacted log for mailto:', e);
			body = `${body}\n\n---\nFailed to attach debug log: ${e}`;
		}
	}

	if (request.attachments?.length) {
		const paths = await Promise.all(request.attachments.map(saveFileToCache));
		attachments = [...(attachments ?? []), ...paths];
	}

	await invoke('plugin:mailto|mailto', {
		request: {
			email: 'support@dashchat.org',
			subject: request.subject,
			body,
			attachments,
		},
	});
}

async function saveFileToCache(file: File): Promise<string> {
	const bytes = new Uint8Array(await file.arrayBuffer());
	const cacheDir = await appCacheDir();
	const shareDir = await join(cacheDir, 'share');
	await mkdir(shareDir, { recursive: true });
	const path = await join(shareDir, file.name);
	await writeFile(path, bytes);
	return path;
}
