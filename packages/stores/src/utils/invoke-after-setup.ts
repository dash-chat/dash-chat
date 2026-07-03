import {
	type InvokeArgs,
	type InvokeOptions,
	invoke as tauriInvoke,
} from '@tauri-apps/api/core';

// Tauri returns this framework error when a command needs managed state that
// hasn't been registered yet. At startup the webview can invoke node-backed
// commands before `async_setup` reaches `app_handle.manage(node)` (the node is
// built before it is managed). The error is transient — retry until ready.
const BACKEND_NOT_READY = 'state not managed';

// The iOS app quiesces its node when backgrounded (releasing SQLite locks to
// avoid a 0xdead10cc kill) and rebuilds it on foreground. While paused/rebuilding,
// node-backed commands return this sentinel (see `AppNode::get`); retry until the
// node is back.
const NODE_NOT_READY = 'NodeNotReady';

// ~15s: covers both the startup race and node rebuild on foreground (iroh/relay
// bring-up). While backgrounded the webview is suspended, so no attempts burn.
const MAX_ATTEMPTS = 150;
const RETRY_DELAY_MS = 100;

const isBackendNotReady = (error: unknown): boolean => {
	if (
		typeof error === 'object' &&
		error !== null &&
		'kind' in error &&
		error.kind === NODE_NOT_READY
	) {
		return true;
	}
	const message =
		typeof error === 'string' ? error : ((error as Error)?.message ?? '');
	return (
		message.includes(BACKEND_NOT_READY) || message.includes(NODE_NOT_READY)
	);
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
