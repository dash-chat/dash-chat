import { invokeAfterSetup } from 'dash-chat-stores';

export type NotificationPermissionState = 'granted' | 'denied' | 'prompt';

// The plugin's isPermissionGranted() answers from a value cached at page load,
// which goes stale once the user changes the permission in the OS settings.
export async function getNotificationPermission(): Promise<NotificationPermissionState> {
	const granted = await invokeAfterSetup<boolean | null>(
		'plugin:notification|is_permission_granted',
	);
	if (granted === null) return 'prompt';
	return granted ? 'granted' : 'denied';
}

export async function requestNotificationPermission(): Promise<NotificationPermissionState> {
	try {
		await invokeAfterSetup('plugin:notification|request_permission');
	} catch (e) {
		// On Android the plugin registers with FCM right after the grant and
		// rejects the whole call when that fails, granted or not.
		console.error('Failed to request notification permission:', e);
	}
	return getNotificationPermission();
}
