import { invokeAfterSetup } from 'dash-chat-stores';

// The plugin's isPermissionGranted() answers from a value cached at page load,
// which goes stale once the user changes the permission in the OS settings.
export async function isNotificationPermissionGranted(): Promise<boolean> {
	const granted = await invokeAfterSetup<boolean | null>(
		'plugin:notification|is_permission_granted',
	);
	return granted === true;
}

export async function requestNotificationPermission(): Promise<void> {
	await invokeAfterSetup('plugin:notification|request_permission');
}
