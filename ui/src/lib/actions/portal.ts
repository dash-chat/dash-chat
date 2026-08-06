import type { Action } from 'svelte/action';

/**
 * Moves the node to the end of `<body>`. For fixed-position content (dialogs,
 * sheets) mounted inside a transformed or hidden ancestor — a Konsta popover,
 * say — which would otherwise become its containing block and clip it.
 */
export const portal: Action = node => {
	document.body.appendChild(node);
	return {
		destroy() {
			node.remove();
		},
	};
};
