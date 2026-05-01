<!--
	A Konsta `<Page>` with a built-in column-reverse chat scroll container.

	Usage:
	  <ReverseScrollPage data-testid="...">
	    {#snippet navbar()}
	      <Navbar transparent />
	    {/snippet}
	    ...messages and overlays...
	  </ReverseScrollPage>

	The `navbar` snippet renders as a sibling of the scroll element inside
	`.k-page`, NOT inside the scroll wrapper. This matters: WebKit has a bug
	where a `position: sticky` element nested inside a scrollable container
	whose ancestor WKWebView frame just resized (e.g. iOS keyboard dismiss)
	leaves the navbar's compositing layer stale — it occupies layout but
	renders nothing until a real touch event forces re-layout. Putting the
	navbar in `.k-page` (which has `overflow: hidden`, so its sticky context
	degenerates to fixed top placement) sidesteps the bug.

	What it does:
	1. Suppresses scroll on `.k-page` (`overflow: hidden`) so the page itself
	   doesn't scroll.
	2. Positions the scroll element as `absolute; inset: 0` inside `.k-page` so
	   the viewport extends from top to bottom — content scrolls *under* the
	   navbar's translucent layers, preserving Konsta's iOS gradient/blur
	   fade-into-background effect.
	3. Makes the scroll element a column-reverse container (scrollTop=0 = bottom).
	4. Tracks the navbar's measured height and exposes it on the scroll element
	   as `--chat-navbar-height`. The inner growth wrapper uses it as
	   `padding-top` so the welcome card / oldest content isn't permanently
	   hidden behind the navbar at max scroll-up. Descendants (e.g. a sticky
	   day-tag) can read it the same way.
	5. Manages the Material navbar bg opacity: opaque at the latest-message end,
	   transparent over the welcome card. iOS isn't touched — Konsta's gradient +
	   blur layers do the fading visually on their own.

	Why not Konsta's `scrollEl` prop: Konsta's progress formula clamps
	`scrollTop ≥ 0`, but WebKit reports negative scrollTop in column-reverse, so
	it would always compute progress=0 and never fade.

	Props mirror Konsta's `<Page>` (Konsta-specific options forwarded to the
	underlying Page). Plain HTML attributes (id, class, style, data-*, aria-*…)
	land on the inner scroll element.
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
		navbar?: Snippet;
		children?: Snippet;
	}

	let {
		component,
		colors,
		ios,
		material,
		isAtBottom = $bindable(true),
		navbar,
		children,
		...scrollProps
	}: Props = $props();

	let el: HTMLDivElement | null = $state(null);
	let innerEl: HTMLDivElement | null = $state(null);
	let suppressCompensateUntil = 0;

	export function scrollToBottom(animate = true) {
		if (!el) return;
		// Suppress scroll compensation for the duration of this scroll: the
		// caller wants the view to land at the bottom, not stay anchored to
		// older messages while the smooth animation runs.
		suppressCompensateUntil = performance.now() + 1000;
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

		const pageEl = node.parentElement;
		if (!pageEl) return;

		let navbarBgEl: HTMLElement | null = null;
		let observedNavbar: HTMLElement | null = null;
		const navbarResizeObserver = new ResizeObserver(() => {
			if (observedNavbar) {
				node.style.setProperty(
					'--chat-navbar-height',
					`${observedNavbar.offsetHeight}px`,
				);
			}
		});

		const syncNavbar = () => {
			const navEl = pageEl.querySelector('.k-navbar') as HTMLElement | null;
			if (navEl !== observedNavbar) {
				if (observedNavbar) navbarResizeObserver.unobserve(observedNavbar);
				observedNavbar = navEl;
				if (navEl) {
					navbarResizeObserver.observe(navEl);
					node.style.setProperty(
						'--chat-navbar-height',
						`${navEl.offsetHeight}px`,
					);
				}
			}
			navbarBgEl = null;
		};

		const updateNavbar = () => {
			// iOS theme: leave the navbar untouched. The gradient + blur fade
			// content into the background visually as it scrolls under them —
			// writing opacity here would defeat that.
			if (node.closest('.k-ios')) return;
			// Re-query if the cached element was removed (e.g. the Navbar got
			// swapped via {#if}{:else}, like toggling search mode in/out).
			if (!navbarBgEl || !navbarBgEl.isConnected) {
				navbarBgEl = findNavbarBg(pageEl);
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
				syncNavbar();
				updateNavbar();
			});
		};

		const updateIsAtBottom = () => {
			// WebKit uses negative scrollTop in column-reverse; abs() normalises.
			isAtBottom = Math.abs(node.scrollTop) < SCROLL_BOTTOM_THRESHOLD;
		};

		// Float shadow of node.scrollTop so successive compensations don't
		// accumulate the browser's per-set rounding error (WKWebView snaps
		// scrollTop to an integer; over N rapid incoming messages each loses
		// up to ~0.5px and the user sees a 1px-ish drift). We do the math in
		// floats and only the final write to node.scrollTop is rounded by the
		// browser; the next compensation builds on the precise float, not the
		// rounded readback.
		let desiredScrollTop = node.scrollTop;

		const onScroll = () => {
			// If the actual scrollTop diverged from our tracked float by more
			// than the rounding margin, treat it as a user scroll and re-sync.
			if (Math.abs(node.scrollTop - desiredScrollTop) > 1) {
				desiredScrollTop = node.scrollTop;
			}
			updateIsAtBottom();
			updateNavbar();
		};
		node.addEventListener('scroll', onScroll);

		updateIsAtBottom();

		// When the user is scrolled up reading older messages and a new message
		// arrives, the inner content grows at its DOM end (visual bottom in
		// column-reverse). The inner div's bottom is anchored to the container,
		// so its top extends upward — every existing message shifts up in
		// absolute terms while scrollTop stays fixed (overflow-anchor: none
		// disables compensation). The result: the messages the user was reading
		// scroll up out of view as newer content slides in from below.
		// Counter this by anchoring to the top of the content: when the inner
		// div grows while the user isn't at the bottom, shift scrollTop by the
		// growth amount so the same messages stay in view.
		//
		// Run the correction synchronously in the MutationObserver microtask
		// (not in a ResizeObserver) so it lands before the render phase. A
		// render-phase fix would mean the rAF scheduled here reads the new
		// scrollHeight with the old scrollTop and paints once with the
		// uncompensated layout — visible as a small scroll flicker before the
		// view resets. Forcing layout via inner.offsetHeight inside the MO
		// gives us the post-mutation height while the rAF and any paint are
		// still pending.
		// getBoundingClientRect().height returns a fractional value reflecting
		// the actual subpixel layout, while offsetHeight returns a rounded
		// integer. With offsetHeight, a real height delta of 30.4px reads as
		// 30, leaving a 0.4px residual visible as a one-pixel up/down jitter
		// after compensation; using the rect's float height keeps the
		// correction in lock-step with the layout.
		let prevInnerHeight = inner.getBoundingClientRect().height;
		const compensateScroll = () => {
			const newInnerHeight = inner.getBoundingClientRect().height;
			const delta = newInnerHeight - prevInnerHeight;
			prevInnerHeight = newInnerHeight;
			if (delta <= 0) return;
			if (performance.now() < suppressCompensateUntil) return;
			if (Math.abs(desiredScrollTop) < SCROLL_BOTTOM_THRESHOLD) return;
			desiredScrollTop -= delta;
			node.scrollTop = desiredScrollTop;
		};

		// Run scroll compensation off mutations inside the scroll content only.
		// Scoping subtree:true to the inner div avoids forcing layout (via
		// getBoundingClientRect inside compensateScroll) on every unrelated
		// mutation under .k-page — search-input keystrokes, sheet/dialog
		// open-close, navbar text updates, etc.
		const innerObserver = new MutationObserver(compensateScroll);
		innerObserver.observe(inner, { childList: true, subtree: true });

		// Backup: a ResizeObserver on the inner div catches any size change the
		// MutationObserver missed (e.g. async-loaded web component content like
		// <wa-relative-time> resolving its rendered text after the initial
		// bubble paint). RO fires in the same render phase, before paint, so
		// it still pre-empts the visible flicker. If the MO already
		// compensated, prevInnerHeight is up to date and this RO call is a
		// no-op.
		const innerResizeObserver = new ResizeObserver(compensateScroll);
		innerResizeObserver.observe(inner);

		// Watch only the direct children of the page for navbar swaps (e.g.
		// when search mode toggles in/out the active <Navbar> element is
		// replaced). subtree:false means typing in the navbar's search input
		// or any other deep mutation doesn't wake this observer up.
		const pageObserver = new MutationObserver(scheduleUpdate);
		pageObserver.observe(pageEl, { childList: true, subtree: false });

		// Re-evaluate when the viewport shrinks/grows (e.g. the iOS keyboard
		// opening resizes the WKWebView frame) — clientHeight changes shift
		// maxScroll, so the opacity formula needs to re-run even when scrollTop
		// itself didn't move.
		const resizeObserver = new ResizeObserver(scheduleUpdate);
		resizeObserver.observe(node);

		syncNavbar();
		updateNavbar();

		return () => {
			if (frame) cancelAnimationFrame(frame);
			innerObserver.disconnect();
			pageObserver.disconnect();
			navbarResizeObserver.disconnect();
			resizeObserver.disconnect();
			innerResizeObserver.disconnect();
			node.removeEventListener('scroll', onScroll);
		};
	});
</script>

<Page {...pageProps} class="reverse-scroll-host">
	{@render navbar?.()}
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
		<div
			bind:this={innerEl}
			style="flex: 1 0 auto; padding-top: var(--chat-navbar-height, 0px);"
		>
			{@render children?.()}
		</div>
	</div>
</Page>

<style>
	/*
		Konsta's `.k-page` is `absolute overflow-auto` by default. We use it
		as a positioning context for our absolute scroll element + sibling
		navbar, but we don't want it to scroll itself.
	*/
	:global(.reverse-scroll-host) {
		overflow: hidden;
	}
</style>
