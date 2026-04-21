import { invoke } from '@tauri-apps/api/core';

interface MailtoRequest {
	subject: string;
	body: string;
	includeDebugLog: boolean;
}

export async function sendMailto(request: MailtoRequest): Promise<void> {
	let attachments: string[] | undefined;
	if (request.includeDebugLog) {
		const redactedPath = await invoke<string>('get_redacted_log');
		attachments = [redactedPath];
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
