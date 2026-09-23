<script lang="ts">
	import { Popover } from 'konsta/svelte';
	import { fade } from 'svelte/transition';
	import { untrack, type Snippet } from 'svelte';
	import { m } from '$lib/paraglide/messages.js';
	import { holdKeyboardSlot } from 'tauri-plugin-virtual-keyboard';
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
		/** Rendered in a pill-shaped popover above the target; omit to show no
		 * popover there. */
		above?: Snippet;
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

	/** Pin `el` out of the flow, fixed at `base` (its pressed position), lock
	 * its holder's height so the list doesn't reflow, and arm its lift
	 * transition. The lift puts the target above the backdrop, so its own
	 * content stays clickable — tapping a photo would open the lightbox behind
	 * the overlay. Clicks are swallowed in the capture phase, so no descendant
	 * handler runs: while spotlighted the message is there to be looked at,
	 * not used. Returns the restore function. */
	function pinTarget(el: HTMLElement, base: DOMRect): () => void {
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
	}

	$effect(() => {
		if (opened) spotlighted = true;
	});

	$effect(() => {
		if (!spotlighted) return;
		return holdKeyboardSlot();
	});

	// The whole spotlight scene lives between the page chrome (z <= 30) and
	// Konsta's modal layer (z-40): backdrop 32, lifted target 34, anchored
	// popovers 36. Sheets/dialogs (40) and toasts (50) always cover it.
	// Pinning fixes the target at its pressed position (plus the bump), so
	// layout shifts underneath — the chat re-anchoring to the bottom when the
	// keyboard hides, scroll compensations — cannot move it. Matches Signal's
	// focused-message lift.
	// Runs once while spotlighted and tears down only when the dim has fully
	// faded — it deliberately does NOT depend on `opened`, so dismissing
	// doesn't re-run it and snap the styles. The transform effect below owns
	// the open/close animation.
	const LIFT_MS = 190;
	$effect(() => {
		if (!spotlighted || !target || !baseRect) return;
		return pinTarget(target, baseRect);
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

	// Hold the popovers back until the target has finished lifting into place,
	// so they scale out from the settled message instead of racing its
	// transition — a CSS enter delay matching LIFT_MS, applied only while
	// showing so dismissal still hides them immediately.
	const panelsShown = $derived(opened && !contentHidden);
	const panelDelay = $derived(panelsShown ? '!delay-[190ms]' : '');

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
		bump = untrack(() =>
			belowEl
				? ensembleBump(base, aboveEl?.offsetHeight ?? 0, belowEl.offsetHeight)
				: 0,
		);
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

	/** The minimal vertical shift that fits the whole ensemble — reaction bar,
	 * target, actions menu — between the safe-area top and the bottom of the
	 * full viewport, which the retracting keyboard uncovers (innerHeight: the
	 * webview never resizes). */
	function ensembleBump(base: DOMRect, aboveH: number, belowH: number): number {
		const { top: safeTop, bottom: safeBottom } = safeAreaInsets();
		const bottomLimit = window.innerHeight - safeBottom;
		let next = 0;
		const menuBottom = base.bottom + GAP + belowH + MARGIN;
		if (menuBottom > bottomLimit) next -= menuBottom - bottomLimit;
		const barTop = base.top + next - GAP - aboveH - MARGIN;
		if (barTop < safeTop) next += safeTop - barTop;
		return next;
	}
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

{#if above}
	<Popover
		opened={panelsShown && aboveAnchor !== undefined}
		target={aboveAnchor}
		backdrop={false}
		class="!z-[36] !w-auto !rounded-full !origin-bottom [&>div]:!translate-y-0 {panelDelay}"
	>
		<div bind:this={aboveEl}>
			{@render above()}
		</div>
	</Popover>
{/if}

<Popover
	opened={panelsShown && belowAnchor !== undefined}
	target={belowAnchor}
	backdrop={false}
	class="!z-[36] !w-auto !min-w-44 !origin-top [&>div]:!rounded-2xl [&>div]:!translate-y-0 {panelDelay}"
>
	<div bind:this={belowEl}>
		{@render below()}
	</div>
</Popover>
