import type { Action } from 'svelte/action';

/**
 * Shrinks a wrapped text block to its widest rendered line. When text wraps,
 * `fit-content` resolves to the max-width cap even though every line ends
 * short of it, leaving dead space after the ragged line ends; CSS cannot
 * re-shrink a block to its wrapped lines, so we measure and set the width.
 */
export const shrinkToWidestLine: Action<HTMLElement> = node => {
	let appliedW = -1;
	let appliedH = -1;
	let resizeTimer: ReturnType<typeof setTimeout> | undefined;

	const fit = () => {
		node.style.width = '';
		const range = document.createRange();
		range.selectNodeContents(node);
		const rects = Array.from(range.getClientRects()).filter(r => r.width > 0);
		if (rects.length === 0) return;
		const width = Math.ceil(
			Math.max(...rects.map(r => r.right)) -
				Math.min(...rects.map(r => r.left)),
		);
		node.style.width = `${width}px`;
		const rect = node.getBoundingClientRect();
		appliedW = Math.round(rect.width);
		appliedH = Math.round(rect.height);
	};

	// Re-fit when the block is resized by anything other than `fit` itself,
	// e.g. clamped by `max-width: 100%` when the chat column narrows, or
	// reflowed taller because the timestamp spacer grew.
	const observer = new ResizeObserver(() => {
		const rect = node.getBoundingClientRect();
		if (
			Math.round(rect.width) !== appliedW ||
			Math.round(rect.height) !== appliedH
		)
			fit();
	});

	// Content changes (e.g. the timestamp spacer unmounting when the message
	// stops being last) don't resize the explicitly-sized box, so the
	// ResizeObserver misses them. Attributes are left unobserved so that
	// `fit` setting the width doesn't retrigger the observer.
	const mutationObserver = new MutationObserver(fit);

	// Widening the window never resizes an already-fitted block, so the
	// observer alone would keep bubbles narrow forever; re-fit after resizes.
	const onWindowResize = () => {
		clearTimeout(resizeTimer);
		resizeTimer = setTimeout(fit, 100);
	};

	fit();
	observer.observe(node);
	mutationObserver.observe(node, {
		childList: true,
		subtree: true,
		characterData: true,
	});
	window.addEventListener('resize', onWindowResize);
	document.fonts.ready.then(fit);

	return {
		destroy() {
			clearTimeout(resizeTimer);
			observer.disconnect();
			mutationObserver.disconnect();
			window.removeEventListener('resize', onWindowResize);
		},
	};
};
