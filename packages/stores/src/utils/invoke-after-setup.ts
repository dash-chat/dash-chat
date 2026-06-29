import {
	type InvokeArgs,
	type InvokeOptions,
	invoke as tauriInvoke,
} from '@tauri-apps/api/core';

// Tauri returns this framework error when a command needs managed state that
// hasn't been registered yet. At startup the webview can invoke node-backed
// commands before `async_setup` reaches `app_handle.manage(node)` (the node is
// built before it is managed). The error is transient — retry briefly until the
// node is managed. Every other error propagates immediately.
const BACKEND_NOT_READY = 'state not managed';

const MAX_ATTEMPTS = 50;
const RETRY_DELAY_MS = 100;

const isBackendNotReady = (error: unknown): boolean => {
	const message =
		typeof error === 'string' ? error : ((error as Error)?.message ?? '');
	return message.includes(BACKEND_NOT_READY);
};

export const sleep = (ms: number): Promise<void> =>
	new Promise(resolve => setTimeout(resolve, ms));

export async function invokeAfterSetup<T>(
	cmd: string,
	args?: InvokeArgs,
	options?: InvokeOptions,
): Promise<T> {
	for (let attempt = 1; ; attempt++) {
		try {
			return await tauriInvoke<T>(cmd, args, options);
		} catch (error) {
			if (!isBackendNotReady(error) || attempt >= MAX_ATTEMPTS) throw error;
			await sleep(RETRY_DELAY_MS);
		}
	}
}
