import { offlineMode } from '$lib/stores/offline-mode.svelte';
import {
	isServiceRunning,
	startService,
	stopService,
} from 'tauri-plugin-background-service';

export function startOfflineModeLifecycle(): () => void {
	const handleVisibilityChange = () => {
		if (!offlineMode.enabled) return;

		if (document.visibilityState === 'hidden') {
			startService({ serviceLabel: 'Dash Chat' }).catch(e => {
				console.error('[background-service] startService failed:', e);
			});
		} else if (document.visibilityState === 'visible') {
			isServiceRunning().then(running => {
				if (!running) return;
				stopService().catch(e => {
					console.error('[background-service] stopService failed:', e);
				});
			});
		}
	};

	document.addEventListener('visibilitychange', handleVisibilityChange);
	return () =>
		document.removeEventListener('visibilitychange', handleVisibilityChange);
}
