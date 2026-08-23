<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import type { Snippet } from 'svelte';
	import { mdiReply } from '@mdi/js';
	import { wrapPathInSvg } from '$lib/utils/icon';

	let {
		onReply,
		children,
	}: {
		onReply?: () => void;
		children: Snippet;
	} = $props();

	const THRESHOLD = 64;
	const ENGAGE_DISTANCE = 10;

	let node = $state<HTMLElement>();
	let offset = $state(0);
	let sign = $state(1);
	let swiping = $state(false);
	let settling = $state(false);
	let startX = 0;
	let startY = 0;
	let tracking = false;

	const progress = $derived(Math.min(offset / THRESHOLD, 1));
	const moving = $derived(swiping || settling);

	/** Damps travel past the trigger threshold so the row resists further pull. */
	function damp(distance: number) {
		if (distance <= THRESHOLD) return distance;
		return THRESHOLD + (distance - THRESHOLD) * 0.3;
	}

	/** Whether the gesture so far is a deliberate start-to-end drag rather than
	 * the beginning of a vertical scroll. */
	function isReplyDrag(dx: number, dy: number) {
		return dx > 0 && Math.abs(dx) > Math.abs(dy) * 1.5;
	}

	function onTouchStart(e: TouchEvent) {
		if (onReply === undefined || e.touches.length !== 1) return;
		startX = e.touches[0].clientX;
		startY = e.touches[0].clientY;
		sign = node && getComputedStyle(node).direction === 'rtl' ? -1 : 1;
		tracking = true;
		settling = false;
	}

	function onTouchMove(e: TouchEvent) {
		if (!tracking) return;
		const dx = (e.touches[0].clientX - startX) * sign;
		const dy = e.touches[0].clientY - startY;
		if (!swiping) {
			if (Math.hypot(dx, dy) < ENGAGE_DISTANCE) return;
			if (!isReplyDrag(dx, dy)) {
				tracking = false;
				return;
			}
			swiping = true;
		}
		e.preventDefault();
		offset = damp(dx);
	}

	function onTouchEnd() {
		if (!tracking) return;
		const triggered = swiping && offset >= THRESHOLD;
		tracking = false;
		settling = swiping;
		swiping = false;
		offset = 0;
		if (triggered) onReply?.();
	}

	// Svelte's delegated touch handlers are passive, so the drag has to claim the
	// gesture from the scroller through a listener registered as non-passive.
	$effect(() => {
		const el = node;
		if (el === undefined) return;
		el.addEventListener('touchstart', onTouchStart, { passive: true });
		el.addEventListener('touchmove', onTouchMove, { passive: false });
		el.addEventListener('touchend', onTouchEnd);
		el.addEventListener('touchcancel', onTouchEnd);
		return () => {
			el.removeEventListener('touchstart', onTouchStart);
			el.removeEventListener('touchmove', onTouchMove);
			el.removeEventListener('touchend', onTouchEnd);
			el.removeEventListener('touchcancel', onTouchEnd);
		};
	});
</script>

<div
	bind:this={node}
	class="swipe-to-reply"
	class:swiping
	data-testid="swipe-to-reply"
>
	{#if swiping}
		<span
			class="swipe-hint"
			style="opacity: {progress}; scale: {0.6 + progress * 0.4}"
			aria-hidden="true"
		>
			<wa-icon src={wrapPathInSvg(mdiReply)} style="font-size: 1.1rem"
			></wa-icon>
		</span>
	{/if}
	<!-- A resting row carries no `translate` at all: any transform would make it
	     the containing block for the `position: fixed` popovers the message
	     mounts, moving the actions menu off screen. -->
	<div
		class="swipe-content"
		class:settling
		style={moving ? `translate: ${offset * sign}px` : ''}
		ontransitionend={() => (settling = false)}
	>
		{@render children()}
	</div>
</div>

<style>
	/* Clipped only while dragging, so the hover toolbar and reaction overlay are
	   not cut off at rest. `clip` keeps the vertical axis visible; `hidden`
	   would force it to scroll. */
	.swipe-to-reply.swiping {
		position: relative;
		overflow-x: clip;
		overflow-y: visible;
	}

	.swipe-hint {
		position: absolute;
		inset-inline-start: 0.75rem;
		top: 50%;
		translate: 0 -50%;
		display: flex;
		pointer-events: none;
		color: var(--color-brand-primary);
	}

	.swipe-content.settling {
		transition: translate 0.18s ease-out;
	}
</style>
