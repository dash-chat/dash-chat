<script lang="ts">
	import { Popover } from 'konsta/svelte';
	import { fade } from 'svelte/transition';
	import type { Snippet } from 'svelte';
	import { m } from '$lib/paraglide/messages.js';
	import {
		holdKeyboardSlot,
		suppressKeyboardRestore,
	} from 'tauri-plugin-virtual-keyboard';
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

	/** Pin `el` out of the flow, fixed at `base` (its pressed position), lock
	 * its holder's height so the list doesn't reflow, and arm its lift
	 * transition. The lift puts the target above the backdrop, so it is made
	 * inert: touches fall through to the backdrop, which dismisses the overlay,
	 * and nothing inside it can be activated behind the overlay. Returns the
	 * restore function. */
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
		el.style.transformOrigin = `top ${edgeNearerScreenEdge(base)}`;
		el.style.transition = `transform ${LIFT_MS}ms ease-out`;
		el.inert = true;
		return () => {
			el.inert = false;
			el.style.position = '';
			el.style.top = '';
			el.style.left = '';
			el.style.width = '';
			el.style.margin = '';
			el.style.zIndex = '';
			el.style.transition = '';
			el.style.transformOrigin = '';
			el.style.transform = '';
			if (holder) holder.style.height = '';
		};
	}

	$effect(() => {
		if (opened) spotlighted = true;
	});

	// A sheet covering the overlay (contentHidden) lays out against the keyboard
	// inset, so the slot is given back while it shows; its restore waits for the
	// overlay to close, as it would have with the slot held.
	$effect(() => {
		if (!spotlighted) return;
		return contentHidden ? suppressKeyboardRestore() : holdKeyboardSlot();
	});

	// The whole spotlight scene lives between the page chrome (z <= 30) and
	// Konsta's modal layer (z-40): backdrop 32, lifted target 34, anchored
	// popovers 36. Sheets/dialogs (40) and toasts (50) always cover it.
	// Pinning fixes the target at its pressed position (plus the lift), so
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
	// per-frame re-layout. Focused = moved to `focusedRect` to fit the ensemble;
	// resting = the pressed spot. Flipping `opened` animates between them — up into
	// focus on open, settle back on close — without re-pinning, so it's symmetric
	// and can't stutter on an image bubble. Only a target too tall for the
	// ensemble is scaled: scaling an image bubble by a non-integer factor filters
	// its edge into a faint vertical seam.
	$effect(() => {
		if (!spotlighted || !target || !baseRect || !focusedRect) return;
		target.style.transform = opened
			? `translateY(${focusedRect.top - baseRect.top}px) scale(${focusedRect.height / baseRect.height})`
			: 'translateY(0px) scale(1)';
	});

	// Hold the popovers back until the target has finished lifting into place,
	// so they scale out from the settled message instead of racing its
	// transition — a CSS enter delay matching LIFT_MS, applied only while
	// showing so dismissal still hides them immediately.
	const panelsShown = $derived(opened && !contentHidden);
	const panelDelay = $derived(panelsShown ? '!delay-[190ms]' : '');

	// Where the target was when it was pressed is the reference the overlay
	// pins the message to, like Signal does — plus the minimal vertical shift,
	// and the shrink of a target too tall for it, that fit the whole ensemble
	// (reaction bar, message, actions menu) inside the full viewport, which
	// the retracting keyboard uncovers (it overlays the webview, so nothing
	// resizes). The pressed position is read once at open, and the fit follows
	// only the popovers' heights; nothing here re-runs while the keyboard
	// animates away.
	let baseRect = $state<DOMRect>();
	let aboveHeight = $state(0);
	let belowHeight = $state(0);

	$effect(() => {
		if (!opened || !target) {
			// Keep the pressed position while the backdrop fades out — the fixed
			// lift needs it until the very end.
			if (!spotlighted) baseRect = undefined;
			return;
		}
		baseRect = target.getBoundingClientRect();
	});

	const focusedRect = $derived(
		baseRect && ensembleTargetRect(baseRect, aboveHeight, belowHeight),
	);

	// Konsta popovers only re-read their anchors on window resize; nudge
	// them once the anchors are in place.
	$effect(() => {
		if (!focusedRect) return;
		requestAnimationFrame(() => window.dispatchEvent(new Event('resize')));
	});

	// Outlives `focusedRect`: the popovers finish closing after the backdrop's
	// fade, and losing it sooner would snap them back to Konsta's centred position.
	let edgeStyle = $state('');
	$effect.pre(() => {
		if (focusedRect) edgeStyle = outerEdgeStyle(focusedRect);
	});

	let aboveAnchor = $state<HTMLElement>();
	let belowAnchor = $state<HTMLElement>();

	const GAP = 8;
	const MARGIN = 8;
	// A viewport too short for the popovers alone leaves no room to fit into.
	const MIN_FOCUSED_HEIGHT = 48;

	/** Where the target sits so the whole ensemble — reaction bar, target,
	 * actions menu — fits between the safe-area top and the bottom of the full
	 * viewport, which the retracting keyboard uncovers (innerHeight: the webview
	 * never resizes): shifted vertically as little as possible, and shrunk
	 * towards its outer edge when it is too tall to fit at full size. */
	function ensembleTargetRect(
		base: DOMRect,
		aboveH: number,
		belowH: number,
	): DOMRect {
		const { top: safeTop, bottom: safeBottom } = safeAreaInsets();
		const topLimit = safeTop + MARGIN + aboveH + GAP;
		const bottomLimit = window.innerHeight - safeBottom - MARGIN - belowH - GAP;
		const room = Math.max(MIN_FOCUSED_HEIGHT, bottomLimit - topLimit);
		const scale = Math.min(1, room / base.height);
		const width = base.width * scale;
		const height = base.height * scale;
		const top = Math.max(topLimit, Math.min(base.top, bottomLimit - height));
		const left =
			edgeNearerScreenEdge(base) === 'left' ? base.left : base.right - width;
		return new DOMRect(left, top, width, height);
	}

	// Physical on purpose: it is read off the viewport, so it already follows
	// the bubble to whichever side RTL puts it.
	function edgeNearerScreenEdge(target: DOMRect): 'left' | 'right' {
		return target.left < window.innerWidth - target.right ? 'left' : 'right';
	}

	/** Lines a popover up with the target's outer edge, the one nearer the screen
	 * edge. Konsta only centres popovers, and writes its `left` after this style,
	 * hence the `!important`. */
	function outerEdgeStyle(target: DOMRect): string {
		return edgeNearerScreenEdge(target) === 'left'
			? `left: ${target.left}px !important`
			: `left: auto !important; right: ${window.innerWidth - target.right}px`;
	}
</script>

{#if opened}
	<!-- active:transform-none: the default button press scale would pull the dim
	     in from the screen edges for a frame, flashing the page around them. -->
	<button
		class="fixed inset-0 z-[32] h-full w-full cursor-default bg-black/90 active:transform-none"
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
{#if focusedRect}
	<div
		bind:this={aboveAnchor}
		class="pointer-events-none fixed"
		style={`left: ${focusedRect.left}px; top: ${focusedRect.top - GAP}px; width: ${focusedRect.width}px; height: 0`}
	></div>
	<div
		bind:this={belowAnchor}
		class="pointer-events-none fixed"
		style={`left: ${focusedRect.left}px; top: ${focusedRect.bottom + GAP}px; width: ${focusedRect.width}px; height: 0; --k-safe-area-top: ${focusedRect.bottom + GAP}px`}
	></div>
{/if}

{#if above}
	<Popover
		opened={panelsShown && aboveAnchor !== undefined}
		target={aboveAnchor}
		backdrop={false}
		style={edgeStyle}
		class="!z-[36] !w-auto !rounded-full !origin-bottom [&>div]:!translate-y-0 {panelDelay}"
	>
		<div bind:offsetHeight={aboveHeight}>
			{@render above()}
		</div>
	</Popover>
{/if}

<Popover
	opened={panelsShown && belowAnchor !== undefined}
	target={belowAnchor}
	backdrop={false}
	style={edgeStyle}
	class="!z-[36] !w-auto !min-w-44 !origin-top [&>div]:!rounded-2xl [&>div]:!translate-y-0 {panelDelay}"
>
	<div bind:offsetHeight={belowHeight}>
		{@render below()}
	</div>
</Popover>
