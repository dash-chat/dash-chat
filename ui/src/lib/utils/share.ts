import { shareText as shareTextNative } from '@choochmeque/tauri-plugin-sharekit-api';

/**
 * True when the error is the sharekit plugin's rejection for the user
 * dismissing the system share sheet without sharing (the same string on every
 * platform).
 */
export function isShareCancelled(error: unknown): boolean {
	const message =
		error instanceof Error
			? error.message
			: typeof error === 'string'
				? error
				: '';
	return message.trim() === 'Share cancelled';
}

/**
 * Opens the native share sheet for the given text. Dismissing the share sheet
 * without sharing is not an error and resolves normally.
 */
export async function shareText(text: string): Promise<void> {
	try {
		await shareTextNative(text);
	} catch (e) {
		if (!isShareCancelled(e)) throw e;
	}
}
