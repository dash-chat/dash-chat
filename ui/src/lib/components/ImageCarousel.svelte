<script lang="ts" generics="T">
	import type { Snippet } from 'svelte';
	import type { HTMLAttributes } from 'svelte/elements';

	interface Props extends HTMLAttributes<HTMLDivElement> {
		items: T[];
		/** Index of the centred slide; updated as the user scrolls. */
		index?: number;
		slide: Snippet<[T, number]>;
		/** Freeze horizontal paging (e.g. while a slide is zoomed). */
		paused?: boolean;
	}

	let {
		items,
		index = $bindable(0),
		slide,
		paused = false,
		class: className = '',
		...rest
	}: Props = $props();

	let carouselEl: HTMLElement | undefined = $state();

	// While a programmatic smooth scroll animates, onScroll must not snap the
	// index to the intermediate nearest slide — that would bounce index back to
	// the origin and flicker the UI. Hold the target until we arrive (or a
	// fallback timeout, in case the exact centre is never reached).
	let scrollTarget: number | null = null;
	let releaseTimer: ReturnType<typeof setTimeout> | undefined;

	// Page via scrollIntoView / bounding-box proximity rather than scrollLeft
	// math: RTL's scrollLeft sign convention differs across engines
	// (Chromium/Gecko go negative, WebKit/iOS stays positive), and these are
	// direction-agnostic.
	export function scrollToIndex(i: number, smooth = true) {
		const target = carouselEl?.children[i] as Element | undefined;
		if (!target) return;
		if (smooth) {
			scrollTarget = i;
			clearTimeout(releaseTimer);
			releaseTimer = setTimeout(() => (scrollTarget = null), 600);
		}
		target.scrollIntoView({
			behavior: smooth ? 'smooth' : 'auto',
			inline: 'center',
			block: 'nearest',
		});
	}

	function nearestIndex(): number {
		if (!carouselEl || carouselEl.clientWidth === 0) return index;
		// The active page is the slide whose centre is closest to the viewport's.
		const viewportCenter =
			carouselEl.getBoundingClientRect().left + carouselEl.clientWidth / 2;
		let nearest = index;
		let nearestDistance = Infinity;
		for (let i = 0; i < carouselEl.children.length; i++) {
			const rect = carouselEl.children[i].getBoundingClientRect();
			const distance = Math.abs(rect.left + rect.width / 2 - viewportCenter);
			if (distance < nearestDistance) {
				nearestDistance = distance;
				nearest = i;
			}
		}
		return nearest;
	}

	function onScroll() {
		const nearest = nearestIndex();
		if (scrollTarget !== null) {
			if (nearest === scrollTarget) {
				scrollTarget = null;
				clearTimeout(releaseTimer);
			}
			return;
		}
		if (nearest !== index && nearest < items.length) index = nearest;
	}
</script>

<div
	bind:this={carouselEl}
	class="image-carousel flex snap-x snap-mandatory {paused
		? 'overflow-hidden'
		: 'overflow-x-auto'} {className}"
	onscroll={onScroll}
	{...rest}
>
	{#each items as item, i (i)}
		<div
			class="flex w-full shrink-0 snap-center snap-always items-center justify-center overflow-hidden"
		>
			{@render slide(item, i)}
		</div>
	{/each}
</div>

<style>
	.image-carousel {
		scrollbar-width: none;
	}
	.image-carousel::-webkit-scrollbar {
		display: none;
	}
</style>
