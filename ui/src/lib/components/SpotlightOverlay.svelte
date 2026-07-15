<script lang="ts">
	import { Popover } from 'konsta/svelte';
	import { fade } from 'svelte/transition';
	import type { Snippet } from 'svelte';
	import { m } from '$lib/paraglide/messages.js';

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

	// Spotlight the target: raise it above the dimming backdrop (z-40), while
	// the anchored popovers sit above it (z-50). Matches Signal's
	// focused-message lift.
	$effect(() => {
		if (!spotlighted || !target) return;
		target.style.position = 'relative';
		target.style.zIndex = '45';
		return () => {
			target.style.position = '';
			target.style.zIndex = '';
		};
	});

	// Anchor rect captured while the overlay is up; the backdrop blocks
	// interaction so the target can't move under it.
	const rect = $derived(
		opened && target ? target.getBoundingClientRect() : undefined,
	);

	let aboveAnchor = $state<HTMLElement>();
	let belowAnchor = $state<HTMLElement>();

	const GAP = 8;
</script>

{#if opened}
	<button
		class="fixed inset-0 z-40 h-full w-full cursor-default bg-black/50"
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
	class="!z-50 !w-auto !rounded-full"
>
	{@render above()}
</Popover>

<Popover
	opened={opened && !contentHidden && belowAnchor !== undefined}
	target={belowAnchor}
	backdrop={false}
	class="!z-50 !w-auto !min-w-44 [&>div]:!rounded-2xl"
>
	{@render below()}
</Popover>
