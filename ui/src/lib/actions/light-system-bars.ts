import { applyThemeSystemBars, setLightSystemBars } from '$lib/utils/theme';
import type { Action } from 'svelte/action';

/**
 * Forces light system bars while the node is mounted, restoring the theme's
 * bars on teardown — for surfaces whose background ignores the app theme.
 */
export const lightSystemBars: Action = () => {
	setLightSystemBars().catch(() => {});
	return {
		destroy() {
			applyThemeSystemBars().catch(() => {});
		},
	};
};
