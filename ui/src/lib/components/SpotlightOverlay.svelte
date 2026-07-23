<script lang="ts">
	import { Popover } from 'konsta/svelte';
	import { fade } from 'svelte/transition';
	import { untrack, type Snippet } from 'svelte';
	import { m } from '$lib/paraglide/messages.js';
	import { keyboard } from 'tauri-plugin-virtual-keyboard';
	import {
		preserveKeyboardSpace,
		releaseKeyboardSpace,
		reopenComposerKeyboard,
	} from '$lib/utils/virtual-keyboard/keyboard-space.svelte';
	import { safeAreaInsets } from '$lib/utils/safe-area';

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

	// Stays true until the backdrop finishes fading out, so the target keeps its
	// place in the stacking order the whole time the dim is visible — dropping it
	// earlier would flash the closing backdrop over the target.
	let spotlighted = $state(false);

	function swallowClick(event: Event) {
		event.preventDefault();
		event.stopPropagation();
	}

	$effect(() => {
		if (opened) spotlighted = true;
	});

	// The keyboard gives way to the overlay: preserving its space opens the
	// composer's below-keyboard surface empty, which retracts the keyboard
	// natively while the surface keeps the input bar pinned in its place.
	// Dismissing the overlay refocuses the composer and re-summons the
	// keyboard into the reserved slot.
	// Tracked with a plain variable and an explicit open→close transition
	// (not an effect cleanup): the effect can re-run spuriously while the
	// overlay stays open, and a cleanup-based restore would re-summon the
	// keyboard mid-hide on every such re-run.
	let restoreKeyboard = false;

	$effect(() => {
		if (opened) {
			if (untrack(() => keyboard.isOpen.value)) {
				restoreKeyboard = true;
				preserveKeyboardSpace();
			}
		} else if (restoreKeyboard) {
			restoreKeyboard = false;
			reopenComposerKeyboard();
		}
	});

	// Destroyed while open (e.g. navigation): drop the composer's keyboard
	// spacer, but don't re-summon the keyboard.
	$effect(() => {
		return () => {
			if (restoreKeyboard) releaseKeyboardSpace();
		};
	});

	// The whole spotlight scene lives between the page chrome (z <= 30) and
	// Konsta's modal layer (z-40): backdrop 32, lifted target 34, anchored
	// popovers 36. Sheets/dialogs (40) and toasts (50) always cover it.
	// The target is taken out of the flow and fixed at its pressed position
	// (plus the bump), so layout shifts underneath — the chat re-anchoring to
	// the bottom when the keyboard hides, scroll compensations — cannot move
	// it, and its parent keeps its height locked so the list doesn't reflow.
	// Matches Signal's focused-message lift.
	// Pin the target out of flow at its pressed spot and arm its transition. Runs
	// once while spotlighted and tears down only when the dim has fully faded — it
	// deliberately does NOT depend on `opened`, so dismissing doesn't re-run it and
	// snap the styles. The transform effect below owns the open/close animation.
	const LIFT_MS = 190;
	$effect(() => {
		if (!spotlighted || !target || !baseRect) return;
		const el = target;
		const base = baseRect;
		const holder = el.parentElement;
		if (holder) {
			holder.style.height = `${holder.getBoundingClientRect().height}px`;
		}
		el.style.position = 'fixed';
		el.style.top = `${base.top}px`;
		el.style.left = `${base.left}px`;
		el.style.width = `${base.width}px`;
		el.style.margin = '0';
		el.style.zIndex = '34';
		el.style.transition = `transform ${LIFT_MS}ms ease-out`;
		// The lift puts the target above the backdrop, so its own content stays
		// clickable — tapping a photo would open the lightbox behind the overlay.
		// Swallowed in the capture phase, so no descendant handler runs: while
		// spotlighted the message is there to be looked at, not used.
		el.addEventListener('click', swallowClick, true);
		return () => {
			el.removeEventListener('click', swallowClick, true);
			el.style.position = '';
			el.style.top = '';
			el.style.left = '';
			el.style.width = '';
			el.style.margin = '';
			el.style.zIndex = '';
			el.style.transition = '';
			el.style.transform = '';
			if (holder) holder.style.height = '';
		};
	});

	// The open/close animation: one GPU-composited transform transition, not a
	// per-frame re-layout. Focused = lifted by `bump` to fit the ensemble; resting
	// = the pressed spot. Flipping `opened` animates between them — up into focus on
	// open, settle back on close — without re-pinning, so it's symmetric and can't
	// stutter on an image bubble. No `scale`: scaling an image bubble by a
	// non-integer factor filters its edge into a faint vertical seam.
	$effect(() => {
		if (!spotlighted || !target) return;
		target.style.transform = opened
			? `translateY(${bump}px)`
			: 'translateY(0px)';
	});

	// Hold the popovers back until the target has finished lifting into place, so
	// they scale out from the settled message instead of racing its transition. A
	// fixed timeout, not `transitionend`: a message that already fits has bump 0 and
	// fires no transition at all. Reset on dismiss so they close with the overlay.
	let liftDone = $state(false);
	$effect(() => {
		if (!opened) {
			liftDone = false;
			return;
		}
		const t = setTimeout(() => (liftDone = true), LIFT_MS);
		return () => clearTimeout(t);
	});

	// Where the target was when it was pressed is the reference the overlay
	// pins the message to, like Signal does — plus the minimal vertical shift
	// that fits the whole ensemble (reaction bar, message, actions menu)
	// inside the full viewport, which the retracting keyboard uncovers (it
	// overlays the webview, so nothing resizes). Both are computed once at
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
				(window.visualViewport?.height ?? window.innerHeight) - safeBottom;
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
		class="fixed inset-0 z-[32] h-full w-full cursor-default bg-black/90"
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
	opened={opened && liftDone && !contentHidden && aboveAnchor !== undefined}
	target={aboveAnchor}
	backdrop={false}
	class="!z-[36] !w-auto !rounded-full !origin-bottom [&>div]:!translate-y-0"
>
	<div bind:this={aboveEl}>
		{@render above()}
	</div>
</Popover>

<Popover
	opened={opened && liftDone && !contentHidden && belowAnchor !== undefined}
	target={belowAnchor}
	backdrop={false}
	class="!z-[36] !w-auto !min-w-44 !origin-top [&>div]:!rounded-2xl [&>div]:!translate-y-0"
>
	<div bind:this={belowEl}>
		{@render below()}
	</div>
</Popover>
