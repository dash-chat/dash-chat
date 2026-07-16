<script lang="ts">
	import { Popover } from 'konsta/svelte';
	import { fade } from 'svelte/transition';
	import { untrack, type Snippet } from 'svelte';
	import { m } from '$lib/paraglide/messages.js';
	import { keyboard } from '$lib/utils/keyboard.svelte';
	import { safeAreaInsets } from '$lib/utils/safe-area';
	import {
		hideKeyboard,
		reopenComposerKeyboard,
	} from '$lib/utils/virtual-keyboard';

	interface Props {
		/** Whether the overlay is showing. */
		opened: boolean;
		/** The element the overlay anchors to and spotlights. */
		target: HTMLElement | undefined;
		onClose: () => void;
		/** Hide the anchored popovers while keeping the backdrop as the single,
		 * steady dim (e.g. while a sheet covers the overlay). */
		contentHidden?: boolean;
		/** Rendered in a pill-shaped popover above the target. */
		above: Snippet;
		/** Rendered in a popover below the target. */
		below: Snippet;
	}

	let {
		opened,
		target,
		onClose,
		contentHidden = false,
		above,
		below,
	}: Props = $props();

	// Stays true until the backdrop finishes fading out, so the target keeps
	// its lift the whole time the dim is visible — dropping it earlier would
	// flash the closing backdrop over the target.
	let spotlighted = $state(false);

	$effect(() => {
		if (opened) spotlighted = true;
	});

	// The keyboard gives way to the overlay — hidden explicitly, since only
	// some WebViews retract it on long-press and only some of the time — and
	// if it was open, dismissing the overlay refocuses the composer and brings
	// it back.
	let restoreKeyboard = false;

	$effect(() => {
		if (opened) {
			restoreKeyboard = untrack(() => keyboard.isOpen);
			if (restoreKeyboard) hideKeyboard();
		} else if (restoreKeyboard) {
			restoreKeyboard = false;
			reopenComposerKeyboard();
		}
	});

	// The whole spotlight scene lives between the page chrome (z <= 30) and
	// Konsta's modal layer (z-40): backdrop 32, lifted target 34, anchored
	// popovers 36. Sheets/dialogs (40) and toasts (50) always cover it.
	// The target is taken out of the flow and fixed at its pressed position
	// (plus the bump), so layout shifts underneath — the chat re-anchoring to
	// the bottom when the keyboard hides, scroll compensations — cannot move
	// it, and its parent keeps its height locked so the list doesn't reflow.
	// Matches Signal's focused-message lift.
	$effect(() => {
		if (!spotlighted || !target || !baseRect) return;
		const base = baseRect;
		const holder = target.parentElement;
		if (holder) {
			holder.style.height = `${holder.getBoundingClientRect().height}px`;
		}
		target.style.position = 'fixed';
		target.style.top = `${base.top}px`;
		target.style.left = `${base.left}px`;
		target.style.width = `${base.width}px`;
		target.style.margin = '0';
		target.style.zIndex = '34';
		let raf = 0;
		if (bump !== 0) {
			// Applied a frame later so the bump animates from the pressed spot.
			raf = requestAnimationFrame(() => {
				target.style.transition = 'top 150ms ease';
				target.style.top = `${base.top + bump}px`;
			});
		}
		return () => {
			cancelAnimationFrame(raf);
			target.style.position = '';
			target.style.top = '';
			target.style.left = '';
			target.style.width = '';
			target.style.margin = '';
			target.style.zIndex = '';
			target.style.transition = '';
			if (holder) holder.style.height = '';
		};
	});

	// Where the target was when it was pressed — that position, not wherever
	// the layout re-anchors it once the keyboard hides, is the reference the
	// overlay pins the message to, like Signal does — plus the minimal
	// vertical shift that fits the whole ensemble (reaction bar, message,
	// actions menu) inside the viewport the overlay ends up on: the current
	// one plus whatever the closing keyboard frees. Both are computed once at
	// open; nothing here re-runs while the keyboard animates away.
	let baseRect = $state<DOMRect>();
	let bump = $state(0);
	let aboveEl = $state<HTMLElement>();
	let belowEl = $state<HTMLElement>();

	$effect(() => {
		if (!opened || !target) {
			// Keep the pressed position while the backdrop fades out — the fixed
			// lift needs it until the very end.
			if (!spotlighted) {
				baseRect = undefined;
				bump = 0;
			}
			return;
		}
		const base = target.getBoundingClientRect();
		baseRect = base;
		bump = untrack(() => {
			if (!aboveEl || !belowEl) return 0;
			const { top: safeTop, bottom: safeBottom } = safeAreaInsets();
			const bottomLimit =
				(window.visualViewport?.height ?? window.innerHeight) +
				keyboard.height -
				safeBottom;
			let next = 0;
			const menuBottom = base.bottom + GAP + belowEl.offsetHeight + MARGIN;
			if (menuBottom > bottomLimit) next -= menuBottom - bottomLimit;
			const barTop = base.top + next - GAP - aboveEl.offsetHeight - MARGIN;
			if (barTop < safeTop) next += safeTop - barTop;
			return next;
		});
		// Konsta popovers only re-read their anchors on window resize; nudge
		// them once the anchors are in place.
		requestAnimationFrame(() => window.dispatchEvent(new Event('resize')));
	});

	// Anchors follow the pinned position, never the live layout.
	const rect = $derived(
		baseRect === undefined
			? undefined
			: bump === 0
				? baseRect
				: new DOMRect(
						baseRect.x,
						baseRect.y + bump,
						baseRect.width,
						baseRect.height,
					),
	);

	let aboveAnchor = $state<HTMLElement>();
	let belowAnchor = $state<HTMLElement>();

	const GAP = 8;
	const MARGIN = 8;
</script>

{#if opened}
	<button
		class="fixed inset-0 z-[32] h-full w-full cursor-default bg-black/50"
		aria-label={m.close()}
		transition:fade={{ duration: 200 }}
		onclick={onClose}
		onoutroend={() => (spotlighted = false)}
	></button>
{/if}

<!-- Zero-height anchors at the target's edges, so Konsta's popover positioning
     places one popover above the target and one below it. Konsta prefers the
     space above whenever the popover fits there; the below anchor zeroes that
     space out via its own --k-safe-area-top to force below-placement. -->
{#if rect}
	<div
		bind:this={aboveAnchor}
		class="pointer-events-none fixed"
		style={`left: ${rect.left}px; top: ${rect.top - GAP}px; width: ${rect.width}px; height: 0`}
	></div>
	<div
		bind:this={belowAnchor}
		class="pointer-events-none fixed"
		style={`left: ${rect.left}px; top: ${rect.bottom + GAP}px; width: ${rect.width}px; height: 0; --k-safe-area-top: ${rect.bottom + GAP}px`}
	></div>
{/if}

<Popover
	opened={opened && !contentHidden && aboveAnchor !== undefined}
	target={aboveAnchor}
	backdrop={false}
	class="!z-[36] !w-auto !rounded-full"
>
	<div bind:this={aboveEl}>
		{@render above()}
	</div>
</Popover>

<Popover
	opened={opened && !contentHidden && belowAnchor !== undefined}
	target={belowAnchor}
	backdrop={false}
	class="!z-[36] !w-auto !min-w-44 [&>div]:!rounded-2xl"
>
	<div bind:this={belowEl}>
		{@render below()}
	</div>
</Popover>
