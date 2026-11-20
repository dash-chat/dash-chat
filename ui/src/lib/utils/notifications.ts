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
