import { invoke } from '@tauri-apps/api/core';

interface MailtoRequest {
	subject: string;
	body: string;
	includeDebugLog: boolean;
	attachments?: File[];
}

export async function sendMailto(request: MailtoRequest): Promise<void> {
	let attachments: string[] | undefined;
	if (request.includeDebugLog) {
		const redactedPath = await invoke<string>('get_redacted_log');
		attachments = [redactedPath];
	}

	if (request.attachments?.length) {
		const paths = await Promise.all(request.attachments.map(saveFileToCache));
		attachments = [...(attachments ?? []), ...paths];
	}

	await invoke('plugin:mailto|mailto', {
		request: {
			email: 'support@dashchat.org',
			subject: request.subject,
			body: request.body,
			attachments,
		},
	});
}

async function saveFileToCache(file: File): Promise<string> {
	const { appCacheDir, join } = await import('@tauri-apps/api/path');
	const { mkdir, writeFile } = await import('@tauri-apps/plugin-fs');

	const bytes = new Uint8Array(await file.arrayBuffer());
	const cacheDir = await appCacheDir();
	const shareDir = await join(cacheDir, 'share');
	await mkdir(shareDir, { recursive: true });
	const path = await join(shareDir, file.name);
	await writeFile(path, bytes);
	return path;
}
