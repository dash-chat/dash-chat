import { invokeAfterSetup } from 'dash-chat-stores';

export async function ensureNotificationPermission(): Promise<boolean> {
	const { isPermissionGranted, requestPermission } = await import(
		'@tauri-apps/plugin-notification'
	);
	if (await isPermissionGranted()) return true;
	try {
		await requestPermission();
	} catch (e) {
		// Once granted, the plugin registers with FCM in the same call and
		// rejects if that fails, although the permission itself was granted.
		console.error('Failed to request notification permission:', e);
	}
	return (await isPermissionGranted()) === true;
}

/** `true` if granted, `false` if denied for good (the OS no longer shows its
 *  dialog, so only the device settings can allow it), `null` if the app can
 *  still ask. */
export function notificationPermissionGranted(): Promise<boolean | null> {
	// Not the plugin's JS `isPermissionGranted`: it keeps the answer of the
	// last request, and folds can-still-ask into denied.
	return invokeAfterSetup<boolean | null>(
		'plugin:notification|is_permission_granted',
	);
}
