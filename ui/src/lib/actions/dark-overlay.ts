import { applyThemeSystemBars, setLightSystemBars } from '$lib/utils/theme';
import type { Action } from 'svelte/action';

/**
 * Marks a full-screen always-dark overlay (lightbox, staged-media page): applies
 * the Konsta dark theme to the node's subtree and forces light system bars while
 * it is mounted, restoring the theme's bars on teardown.
 */
export const darkOverlay: Action = node => {
	node.classList.add('dark');
	setLightSystemBars().catch(() => {});
	return {
		destroy() {
			applyThemeSystemBars().catch(() => {});
		},
	};
};
