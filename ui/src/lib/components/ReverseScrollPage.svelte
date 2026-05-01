<!--
	A Konsta `<Page>` with a built-in column-reverse chat scroll container.

	Usage:
	  <ReverseScrollPage bind:el={scrollEl} data-testid="...">
	    <Navbar transparent />
	    ...messages and overlays...
	  </ReverseScrollPage>

	Children render inside the scroll wrapper. Place a Konsta `<Navbar>` first
	— its built-in `position: sticky; top: 0` keeps it pinned at the viewport
	top while content scrolls underneath in column-reverse order (latest
	message at the bottom, welcome card at the top).

	What it does:
	1. Suppresses scroll on `.k-page` (`overflow: hidden`) via a scoped class
	   so the page itself doesn't scroll.
	2. Positions the scroll element as `absolute; inset: 0` inside `.k-page` so
	   the viewport extends from top to bottom.
	3. Makes the scroll element a column-reverse container (scrollTop=0 = bottom).
	4. An inner `flex: 1 0 auto` wrapper ensures children fill the viewport even
	   when content is short — without this the sticky navbar would sit at the
	   wrapper's natural top, which in column-reverse with under-viewport content
	   is somewhere in the middle of the screen.
	5. Manages the Material navbar bg opacity: opaque at the latest-message end,
	   transparent over the welcome card. iOS isn't touched — Konsta's gradient +
	   blur layers do the fading visually on their own.

	Why not Konsta's `scrollEl` prop: Konsta's progress formula clamps
	`scrollTop ≥ 0`, but WebKit reports negative scrollTop in column-reverse, so
	it would always compute progress=0 and never fade.

	Props mirror Konsta's `<Page>` (Konsta-specific options forwarded to the
	underlying Page). Plain HTML attributes (id, class, style, data-*, aria-*…)
	land on the inner scroll element — that's the element the consumer
	interacts with via `bind:el`.
-->
<script lang="ts">
	import { Page } from 'konsta/svelte';
	import type { Snippet } from 'svelte';
	import type { HTMLAttributes } from 'svelte/elements';
	import { SCROLL_BOTTOM_THRESHOLD } from '$lib/utils/chat';
	import { findNavbarBg } from '$lib/utils/konsta';

	interface PageColors {
		bgIos?: string;
		bgMaterial?: string;
	}

	interface Props extends HTMLAttributes<HTMLDivElement> {
		component?: string;
		colors?: PageColors;
		ios?: boolean;
		material?: boolean;
		isAtBottom?: boolean;
		children?: Snippet;
	}

	let {
		component,
		colors,
		ios,
		material,
		isAtBottom = $bindable(true),
		children,
		...scrollProps
	}: Props = $props();

	let el: HTMLDivElement | null = $state(null);
	let innerEl: HTMLDivElement | null = $state(null);

	export function scrollToBottom(animate = true) {
		if (!el) return;
		el.scrollTo({ top: 0, behavior: animate ? 'smooth' : 'auto' });
	}

	const pageProps: Record<string, unknown> = $derived({
		component,
		colors,
		ios,
		material,
	});

	$effect(() => {
		const node = el;
		if (!node) return;
		const inner = innerEl;
		if (!inner) return;

		let navbarBgEl: HTMLElement | null = null;

		const updateNavbar = () => {
			// iOS theme: leave the navbar untouched. The gradient + blur fade
			// content into the background visually as it scrolls under them —
			// writing opacity here would defeat that.
			if (node.closest('.k-ios')) return;
			// Re-query if the cached element was removed (e.g. the Navbar got
			// swapped via {#if}{:else}, like toggling search mode in/out).
			if (!navbarBgEl || !navbarBgEl.isConnected) {
				navbarBgEl = findNavbarBg(node);
			}
			if (!navbarBgEl) return;

			// In column-reverse, scrollTop=0 is the visual bottom — the latest
			// message sits right under the navbar, so the bg should be opaque to
			// keep them visually separated. As the user scrolls up toward the top
			// of the content (welcome card / avatar area), the bg fades out so the
			// navbar blends with the welcome surface.
			// WebKit uses negative scrollTop in column-reverse; abs() normalises.
			const maxScroll = node.scrollHeight - node.clientHeight;
			navbarBgEl.style.opacity =
				maxScroll < 1
					? '0'
					: maxScroll - Math.abs(node.scrollTop) > 10
						? '1'
						: '0';
		};

		// Coalesce mutation-driven updates to one per frame.
		let frame = 0;
		const scheduleUpdate = () => {
			if (frame) return;
			frame = requestAnimationFrame(() => {
				frame = 0;
				updateNavbar();
			});
		};

		const updateIsAtBottom = () => {
			// WebKit uses negative scrollTop in column-reverse; abs() normalises.
			isAtBottom = Math.abs(node.scrollTop) < SCROLL_BOTTOM_THRESHOLD;
		};

		const onScroll = () => {
			updateIsAtBottom();
			updateNavbar();
		};
		node.addEventListener('scroll', onScroll);

		updateIsAtBottom();

		// Watch the inner flex wrapper for child swaps (e.g. the Navbar being
		// replaced when search mode toggles) so we can re-resolve the navbar bg
		// on the next frame. subtree: false because the navbar is a direct
		// child of this wrapper — narrower than the full scroll subtree, which
		// would wake on every message bubble add, reaction toggle, etc.
		const contentObserver = new MutationObserver(scheduleUpdate);
		contentObserver.observe(inner, { childList: true, subtree: false });

		// Re-evaluate when the viewport shrinks/grows (e.g. the iOS keyboard
		// opening resizes the WKWebView frame) — clientHeight changes shift
		// maxScroll, so the opacity formula needs to re-run even when scrollTop
		// itself didn't move.
		const resizeObserver = new ResizeObserver(scheduleUpdate);
		resizeObserver.observe(node);

		updateNavbar();

		return () => {
			if (frame) cancelAnimationFrame(frame);
			contentObserver.disconnect();
			resizeObserver.disconnect();
			node.removeEventListener('scroll', onScroll);
		};
	});
</script>

<Page {...pageProps} class="reverse-scroll-host">
	<!--
		overflow-anchor: none — disables browser scroll anchoring. WebKit
		otherwise re-pins the scroll position to the visual bottom whenever new
		content is appended in a column-reverse container, even when the user has
		scrolled up to read older messages.
	-->
	<div
		bind:this={el}
		class="absolute inset-0 flex flex-col-reverse overflow-y-auto"
		style="overflow-anchor: none"
		{...scrollProps}
	>
		<div bind:this={innerEl} style="flex: 1 0 auto;">
			{@render children?.()}
		</div>
	</div>
</Page>

<style>
	/*
		Konsta's `.k-page` is `absolute overflow-auto` by default. We use it
		as a positioning context for our absolute scroll element + overlay,
		but we don't want it to scroll itself.
	*/
	:global(.reverse-scroll-host) {
		overflow: hidden;
	}
</style>
