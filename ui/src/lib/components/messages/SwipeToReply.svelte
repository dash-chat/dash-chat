<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { getContext, type Snippet } from 'svelte';
	import { mdiReplyOutline } from '@mdi/js';
	import type { MessagesStore } from 'dash-chat-stores';
	import { useReactiveValue } from '$lib/stores/use-signal';
	import { wrapPathInSvg } from '$lib/utils/icon';

	let {
		onReply,
		target,
		children,
	}: {
		onReply?: () => void;
		/** The element that slides during the gesture — the bubble plus its
		 * reactions, excluding static row chrome like the sender avatar. The
		 * gesture is inert until it is set. */
		target?: HTMLElement;
		children: Snippet;
	} = $props();

	const store: MessagesStore = getContext('messages-store');
	// Read synchronously rather than via {#await}: the gate sits in a touch
	// handler, out of reach of markup awaits. Default to read-only while the
	// value is pending so the gesture fails closed, like the {#await}-gated
	// surfaces.
	const readOnlyChat = useReactiveValue(store.readOnly);
	const readOnly = $derived($readOnlyChat ?? true);

	// Distances mirror Signal Android's ConversationSwipeAnimationHelper
	// (TRIGGER_DX = 64dp, icon slide = 10dp).
	const TRIGGER = 64;
	const ICON_SLIDE = 10;
	const ENGAGE_DISTANCE = 10;
	// Keep in sync with the .swipe-hint width/height below.
	const HINT_SIZE = 38;

	let node = $state<HTMLElement>();
	let dragX = $state(0);
	let sign = $state(1);
	let swiping = $state(false);
	let settling = $state(false);
	let bounced = $state(false);
	let hintStart = $state(0);
	let hintTop = $state(0);
	let startX = 0;
	let startY = 0;
	let tracking = false;

	const progress = $derived(Math.min(dragX / TRIGGER, 1));
	const moving = $derived(swiping || settling);
	const offset = $derived(Math.min(dragX, TRIGGER));
	const hintOpacity = $derived(settling || progress <= 0.05 ? 0 : progress);
	const hintSlide = $derived(settling ? 0 : progress * ICON_SLIDE);

	/** Whether the gesture so far is a deliberate start-to-end drag rather than
	 * the beginning of a vertical scroll. */
	function isReplyDrag(dx: number, dy: number) {
		return dx > 0 && Math.abs(dx) > Math.abs(dy) * 1.5;
	}

	/** Rests the hint at the target's leading edge, vertically centered on it,
	 * like Signal: the target slides away and reveals the icon in the space it
	 * vacated. */
	function placeHint() {
		if (node === undefined || target === undefined) return;
		const rowRect = node.getBoundingClientRect();
		const targetRect = target.getBoundingClientRect();
		hintStart =
			sign === -1
				? rowRect.right - targetRect.right
				: targetRect.left - rowRect.left;
		hintTop =
			targetRect.top - rowRect.top + (targetRect.height - HINT_SIZE) / 2;
	}

	function onTouchStart(e: TouchEvent) {
		if (
			readOnly ||
			onReply === undefined ||
			target === undefined ||
			e.touches.length !== 1
		)
			return;
		startX = e.touches[0].clientX;
		startY = e.touches[0].clientY;
		sign = node && getComputedStyle(node).direction === 'rtl' ? -1 : 1;
		tracking = true;
		settling = false;
		bounced = false;
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
			placeHint();
			swiping = true;
		}
		e.preventDefault();
		dragX = Math.max(dx, 0);
		if (!bounced && dragX >= TRIGGER) {
			bounced = true;
			navigator.vibrate?.(10);
		}
	}

	function onTouchEnd() {
		if (!tracking) return;
		const triggered = swiping && dragX >= TRIGGER;
		tracking = false;
		// A target released at the origin has nothing to animate, so no
		// transitionend would ever clear the settling state.
		settling = swiping && dragX > 0;
		swiping = false;
		dragX = 0;
		if (triggered) onReply?.();
	}

	function onSettled(e: TransitionEvent) {
		if (e.target === target) settling = false;
	}

	// The slide is styled imperatively: the target lives inside the children
	// snippet, out of reach of scoped CSS. A resting target carries no
	// `translate` at all — any transform would make it the containing block for
	// the `position: fixed` popovers the message mounts, moving the actions
	// menu off screen.
	$effect(() => {
		const el = target;
		if (el === undefined) return;
		el.addEventListener('transitionend', onSettled);
		return () => el.removeEventListener('transitionend', onSettled);
	});

	$effect(() => {
		const el = target;
		if (el === undefined) return;
		if (moving) {
			el.style.transition = settling
				? 'translate 0.25s cubic-bezier(0, 0, 0.2, 1)'
				: '';
			el.style.translate = `${offset * sign}px`;
		} else {
			el.style.transition = '';
			el.style.translate = '';
		}
	});

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
	class:moving
	data-testid="swipe-to-reply"
>
	{#if moving}
		<span
			class="swipe-hint quiet"
			class:settling
			class:bounce={bounced}
			style="inset-inline-start: {hintStart}px; top: {hintTop}px; opacity: {hintOpacity}; translate: {hintSlide *
				sign}px; scale: {1 + 0.2 * progress}"
			aria-hidden="true"
		>
			<wa-icon src={wrapPathInSvg(mdiReplyOutline)} style="font-size: 1.25rem"
			></wa-icon>
		</span>
	{/if}
	{@render children()}
</div>

<style>
	/* Clipped only while dragging, so the hover toolbar and reaction overlay are
	   not cut off at rest. `clip` keeps the vertical axis visible; `hidden`
	   would force it to scroll. */
	.swipe-to-reply.moving {
		position: relative;
		overflow-x: clip;
		overflow-y: visible;
	}

	/* Signal's reply affordance: a 38px circular area, glyph centered,
	   vertically centered on the target (top set inline by placeHint). */
	.swipe-hint {
		position: absolute;
		width: 38px;
		height: 38px;
		display: flex;
		align-items: center;
		justify-content: center;
		pointer-events: none;
	}

	.swipe-hint.settling {
		transition:
			opacity 0.25s ease-out,
			translate 0.25s ease-out;
	}

	.swipe-hint.bounce {
		animation: reply-bounce 0.2s ease-in-out;
	}

	@keyframes reply-bounce {
		0% {
			scale: 1.2;
		}
		50% {
			scale: 1.8;
		}
		100% {
			scale: 1.2;
		}
	}
</style>
